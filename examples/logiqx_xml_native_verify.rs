//! Verify one original Logiqx XML document against a published native snapshot.
//!
//! No source document is loaded from the database. The caller supplies the original.

#[path = "support/digest_verify.rs"]
mod digest_verify;
#[path = "support/xml_verify.rs"]
mod xml_verify;

use xml_verify::compare_media_positions;
mod support;

use std::collections::HashMap;
use std::error::Error;
use std::fmt::Debug;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::support::catalog_verify::{VerifyResult, equal, open_existing_catalog};
use camino::Utf8PathBuf;
use mame_coalesce::catalog_files::{
    CatalogFileOccurrence, DigestAlgorithm, LogiqxDiskPayload, LogiqxDumpStatus, LogiqxFilePayload,
    LogiqxRomPayload, LogiqxSamplePayload, occurrences_for_ids,
};
use mame_coalesce::catalog_logiqx::{
    LogiqxBiosSet, LogiqxComment, LogiqxGame, LogiqxPage, LogiqxPageLimit, LogiqxRelease,
    LogiqxYesNo, logiqx_for_snapshot,
};
use mame_coalesce::database::Database;
use mame_coalesce::domain::{OccurrenceId, SnapshotKey};
use mame_coalesce::logiqx::{
    self, AttributePosition, ClrMameProOptions, Disk, Game, GameTextField, Header, HeaderTextField,
    LocatedGame, LogiqxMode, RecordLocation, Rom, RomCenterOptions, Sample,
};

const PAGE_SIZE: usize = 64;
const OCCURRENCE_BATCH_SIZE: usize = 256;

fn main() -> VerifyResult {
    let mut args = std::env::args().skip(1);
    let database_path = Utf8PathBuf::from(args.next().ok_or("missing DATABASE")?);
    let snapshot: SnapshotKey = args.next().ok_or("missing SNAPSHOT_KEY")?.parse()?;
    let source_path = args.next().ok_or("missing SOURCE_XML")?;
    let mode = match args.next().as_deref() {
        None | Some("observed-compatible") => LogiqxMode::ObservedCompatible,
        Some("strict-dtd15") => LogiqxMode::StrictDtd15,
        Some(_) => return Err("MODE must be observed-compatible or strict-dtd15".into()),
    };
    if args.next().is_some() {
        return Err("expected DATABASE SNAPSHOT_KEY SOURCE_XML [MODE]".into());
    }
    verify_path(&database_path, &snapshot, Path::new(&source_path), mode)
}

fn verify_path(
    database_path: &Utf8PathBuf,
    snapshot: &SnapshotKey,
    source_path: &Path,
    mode: LogiqxMode,
) -> VerifyResult {
    let database = open_existing_catalog(database_path)?;

    // The public Logiqx reader accepts a byte slice. Keep this one required source buffer;
    // parsing itself streams each game once and validates through EOF before returning.
    let mut source_bytes = Vec::new();
    File::open(source_path)?.read_to_end(&mut source_bytes)?;

    let first = logiqx_for_snapshot(&database, snapshot, None, LogiqxPageLimit::new(PAGE_SIZE)?)?;
    ensure_page_identity(&first, snapshot)?;
    ensure_mode(&first, mode)?;

    let validated = logiqx::read_with_mode::<_, Box<dyn Error>>(
        &source_bytes,
        mode,
        |_| Verifier::new(&database, snapshot, mode, first),
        |verifier, source_game| verifier.compare_game(&source_game),
    )?;
    let (metadata, mut verifier) = validated.into_parts();
    verifier.compare_document(&metadata)?;
    verifier.finish()?;

    println!(
        "VERIFIED snapshot={} mode={mode:?} games={} media={} header_children={}",
        snapshot.as_str(),
        verifier.games,
        verifier.media,
        verifier.header_children,
    );
    println!(
        "Memory: original/decoded source buffers and coordinate bookkeeping remain input-dependent, plus one parsed game and one {PAGE_SIZE}-game native page with its complete media payloads; occurrence SQL requests are chunked at {OCCURRENCE_BATCH_SIZE}."
    );
    println!("No source-size or game-count cap is imposed by this example.");
    println!(
        "Hash rule: compare Logiqx ROM/disk declared hash text where the public payload retains it; compare root <sha1> as decoded binary because native document metadata is binary."
    );
    Ok(())
}

struct Verifier<'db> {
    database: &'db Database,
    snapshot: &'db SnapshotKey,
    mode: LogiqxMode,
    page: LogiqxPage,
    page_files: HashMap<OccurrenceId, CatalogFileOccurrence>,
    page_index: usize,
    games: usize,
    media: usize,
    header_children: usize,
}

impl<'db> Verifier<'db> {
    fn new(
        database: &'db Database,
        snapshot: &'db SnapshotKey,
        mode: LogiqxMode,
        page: LogiqxPage,
    ) -> VerifyResult<Self> {
        let page_files = load_page_media(database, &page)?;
        Ok(Self {
            database,
            snapshot,
            mode,
            page,
            page_files,
            page_index: 0,
            games: 0,
            media: 0,
            header_children: 0,
        })
    }

    fn compare_game(&mut self, located: &LocatedGame) -> VerifyResult {
        if self.page_index == self.page.games.len() {
            let cursor = self
                .page
                .next_cursor
                .as_ref()
                .ok_or("source contains a game beyond the final native page")?
                .clone();
            let next_page = logiqx_for_snapshot(
                self.database,
                self.snapshot,
                Some(&cursor),
                LogiqxPageLimit::new(PAGE_SIZE)?,
            )?;
            let next_page_files = load_page_media(self.database, &next_page)?;
            self.page = next_page;
            self.page_files = next_page_files;
            self.page_index = 0;
            ensure_page_identity(&self.page, self.snapshot)?;
            ensure_mode(&self.page, self.mode)?;
        }
        let native = self
            .page
            .games
            .get(self.page_index)
            .ok_or("native page cursor returned no game")?;
        compare_game(located, native, self.games, &self.page_files)?;
        self.media += native.media.len();
        self.games += 1;
        self.page_index += 1;
        Ok(())
    }

    fn compare_document(&mut self, source: &logiqx::DocumentMetadata) -> VerifyResult {
        ensure_page_identity(&self.page, self.snapshot)?;
        ensure_mode(&self.page, self.mode)?;
        compare_document_metadata(source, &self.page)?;
        self.header_children = self
            .page
            .document
            .header
            .as_ref()
            .map_or(0, |header| header.text_positions.len());
        Ok(())
    }

    fn finish(&self) -> VerifyResult {
        if self.page_index != self.page.games.len() {
            return Err(format!(
                "source ended after {} games; native page still has {} game(s)",
                self.games,
                self.page.games.len() - self.page_index
            )
            .into());
        }
        if self.page.next_cursor.is_some() {
            return Err(format!(
                "source ended after {} games; native snapshot has another page",
                self.games
            )
            .into());
        }
        Ok(())
    }
}

fn ensure_page_identity(page: &LogiqxPage, snapshot: &SnapshotKey) -> VerifyResult {
    equal("snapshot.key", &page.snapshot.snapshot_key, snapshot)?;
    equal("snapshot.format", &page.snapshot.format.as_str(), &"logiqx")
}

fn ensure_mode(page: &LogiqxPage, mode: LogiqxMode) -> VerifyResult {
    let expected = match mode {
        LogiqxMode::ObservedCompatible => "logiqx-declared-text-compat-v2",
        LogiqxMode::StrictDtd15 => "logiqx-dtd-1.5-v1",
    };
    equal(
        "snapshot.interpretation",
        &page.snapshot.rules_version.as_deref(),
        &Some(expected),
    )
}

fn load_page_media(
    database: &Database,
    page: &LogiqxPage,
) -> VerifyResult<HashMap<OccurrenceId, CatalogFileOccurrence>> {
    let ids = page
        .games
        .iter()
        .flat_map(|game| game.media.iter().map(|media| media.occurrence_id))
        .collect::<Vec<_>>();
    let mut files = Vec::with_capacity(ids.len());
    for batch in ids.chunks(OCCURRENCE_BATCH_SIZE) {
        files.extend(occurrences_for_ids(database, batch)?);
    }
    equal("page.media_payload_count", &files.len(), &ids.len())?;
    let files = files
        .into_iter()
        .map(|file| (file.occurrence_id, file))
        .collect::<HashMap<_, _>>();
    equal("page.unique_media_payload_count", &files.len(), &ids.len())?;
    Ok(files)
}

fn compare_document_metadata(source: &logiqx::DocumentMetadata, page: &LogiqxPage) -> VerifyResult {
    let native = &page.document;
    equal("document.build", &source.build(), &native.build.as_deref())?;
    equal(
        "document.debug_effective",
        &source.debug_effective(),
        &yes_no(native.debug),
    )?;
    equal(
        "document.debug_presence",
        &source.debug_was_explicit(),
        &native.debug_was_present,
    )?;
    equal(
        "document.file_name",
        &source.file_name(),
        &native.file_name.as_deref(),
    )?;
    // The schema intentionally exposes this field as binary, so spelling/case is not asserted.
    equal(
        "document.sha1_bytes",
        &source.sha1(),
        &native.sha1.as_deref(),
    )?;
    compare_positions(
        "document.attributes",
        source.attribute_positions(),
        &native.attribute_positions,
    )?;

    let source_header = source.header_opt();
    let source_version = source_header
        .and_then(|header| header.version())
        .map(String::as_str);
    equal(
        "snapshot.declared_version",
        &source_version,
        &page.snapshot.declared_version.as_deref(),
    )?;
    equal(
        "document.header_presence",
        &source_header.is_some(),
        &native.header.is_some(),
    )?;
    if let (Some(source_header), Some(native_header)) = (source_header, native.header.as_ref()) {
        compare_header(
            source_header,
            native_header,
            native.clrmamepro.as_ref(),
            native.romcenter.as_ref(),
        )?;
    }
    Ok(())
}

fn compare_header(
    source: &Header,
    native: &mame_coalesce::catalog_logiqx::LogiqxHeader,
    native_clrmamepro: Option<&mame_coalesce::catalog_logiqx::LogiqxClrMameProOptions>,
    native_romcenter: Option<&mame_coalesce::catalog_logiqx::LogiqxRomCenterOptions>,
) -> VerifyResult {
    for (name, left, right) in [
        ("name", Some(source.name()), native.name.as_deref()),
        (
            "description",
            source.description().map(String::as_str),
            native.description.as_deref(),
        ),
        (
            "category",
            source.category().map(String::as_str),
            native.category.as_deref(),
        ),
        (
            "version",
            source.version().map(String::as_str),
            native.version.as_deref(),
        ),
        (
            "date",
            source.date().map(String::as_str),
            native.date.as_deref(),
        ),
        (
            "author",
            source.author().map(String::as_str),
            native.author.as_deref(),
        ),
        (
            "email",
            source.email().map(String::as_str),
            native.email.as_deref(),
        ),
        (
            "homepage",
            source.homepage().map(String::as_str),
            native.homepage.as_deref(),
        ),
        (
            "url",
            source.url().map(String::as_str),
            native.url.as_deref(),
        ),
        (
            "comment",
            source.comment().map(String::as_str),
            native.comment.as_deref(),
        ),
    ] {
        equal(&format!("header.{name}"), &left, &right)?;
    }
    let source_positions = source
        .text_positions()
        .iter()
        .map(|p| {
            Ok((
                p.field,
                i64::try_from(p.source_order)?,
                p.location.line,
                p.location.column,
            ))
        })
        .collect::<VerifyResult<Vec<_>>>()?;
    let native_positions = native
        .text_positions
        .iter()
        .map(|p| {
            Ok((
                p.field,
                i64::try_from(p.source_order)?,
                p.location.line,
                p.location.column,
            ))
        })
        .collect::<VerifyResult<Vec<_>>>()?;
    equal(
        "header.text_positions",
        &source_positions,
        &native_positions,
    )?;

    compare_option_presence(
        "header.clrmamepro",
        source.clrmamepro_options(),
        native_clrmamepro,
    )?;
    if let (Some(source_options), Some(native_options)) =
        (source.clrmamepro_options(), native_clrmamepro)
    {
        compare_clrmamepro(source, source_options, native_options)?;
    }
    compare_option_presence(
        "header.romcenter",
        source.romcenter_options(),
        native_romcenter,
    )?;
    if let (Some(source_options), Some(native_options)) =
        (source.romcenter_options(), native_romcenter)
    {
        compare_romcenter(source, source_options, native_options)?;
    }
    compare_header_child_order(source, native, native_clrmamepro, native_romcenter)
}

fn compare_header_child_order(
    source: &Header,
    native: &mame_coalesce::catalog_logiqx::LogiqxHeader,
    native_clrmamepro: Option<&mame_coalesce::catalog_logiqx::LogiqxClrMameProOptions>,
    native_romcenter: Option<&mame_coalesce::catalog_logiqx::LogiqxRomCenterOptions>,
) -> VerifyResult {
    let mut source_children = source
        .text_positions()
        .iter()
        .map(|p| Ok((i64::try_from(p.source_order)?, HeaderChild::Text(p.field))))
        .collect::<VerifyResult<Vec<_>>>()?;
    if let Some(options) = source.clrmamepro_options() {
        source_children.push((
            i64::try_from(
                source
                    .child_source_order(options.location())
                    .ok_or("missing ClrMamePro child order")?,
            )?,
            HeaderChild::ClrMamePro,
        ));
    }
    if let Some(options) = source.romcenter_options() {
        source_children.push((
            i64::try_from(
                source
                    .child_source_order(options.location())
                    .ok_or("missing RomCenter child order")?,
            )?,
            HeaderChild::RomCenter,
        ));
    }
    source_children.sort_by_key(|child| child.0);
    let mut native_children = native
        .text_positions
        .iter()
        .map(|p| Ok((i64::try_from(p.source_order)?, HeaderChild::Text(p.field))))
        .collect::<VerifyResult<Vec<_>>>()?;
    if let Some(options) = native_clrmamepro {
        native_children.push((options.source_order, HeaderChild::ClrMamePro));
    }
    if let Some(options) = native_romcenter {
        native_children.push((options.source_order, HeaderChild::RomCenter));
    }
    native_children.sort_by_key(|child| child.0);
    equal("header.children_order", &source_children, &native_children)
}

#[derive(Debug, PartialEq, Eq)]
enum HeaderChild {
    Text(HeaderTextField),
    ClrMamePro,
    RomCenter,
}

fn compare_option_presence<T, U>(
    field: &str,
    source: Option<&T>,
    native: Option<&U>,
) -> VerifyResult {
    equal(field, &source.is_some(), &native.is_some())
}

fn compare_clrmamepro(
    _header: &Header,
    source: &ClrMameProOptions,
    native: &mame_coalesce::catalog_logiqx::LogiqxClrMameProOptions,
) -> VerifyResult {
    equal(
        "header.clrmamepro.header",
        &source.header(),
        &native.header.as_deref(),
    )?;
    compare_option(
        "forcemerging",
        source.forcemerging(),
        &native.forcemerging,
        source.forcemerging_was_explicit(),
        |v| match v {
            mame_coalesce::catalog_logiqx::LogiqxForceMerging::Split => "split",
            mame_coalesce::catalog_logiqx::LogiqxForceMerging::None => "none",
            mame_coalesce::catalog_logiqx::LogiqxForceMerging::Full => "full",
        },
    )?;
    compare_option(
        "forcenodump",
        source.forcenodump(),
        &native.forcenodump,
        source.forcenodump_was_explicit(),
        |v| match v {
            mame_coalesce::catalog_logiqx::LogiqxForceNoDump::Obsolete => "obsolete",
            mame_coalesce::catalog_logiqx::LogiqxForceNoDump::Required => "required",
            mame_coalesce::catalog_logiqx::LogiqxForceNoDump::Ignore => "ignore",
        },
    )?;
    compare_option(
        "forcepacking",
        source.forcepacking(),
        &native.forcepacking,
        source.forcepacking_was_explicit(),
        |v| match v {
            mame_coalesce::catalog_logiqx::LogiqxForcePacking::Zip => "zip",
            mame_coalesce::catalog_logiqx::LogiqxForcePacking::Unzip => "unzip",
        },
    )?;
    equal(
        "header.clrmamepro.location",
        &source.location(),
        &native.location,
    )?;
    compare_positions(
        "header.clrmamepro.attributes",
        source.attribute_positions(),
        &native.attribute_positions,
    )
}

fn compare_romcenter(
    _header: &Header,
    source: &RomCenterOptions,
    native: &mame_coalesce::catalog_logiqx::LogiqxRomCenterOptions,
) -> VerifyResult {
    equal(
        "header.romcenter.plugin",
        &source.plugin(),
        &native.plugin.as_deref(),
    )?;
    for (name, source_value, native_value, source_present, native_present) in [
        (
            "rommode",
            source.rommode(),
            rom_mode(*native.rommode.effective()),
            source.rommode_was_explicit(),
            native.rommode.was_present(),
        ),
        (
            "biosmode",
            source.biosmode(),
            rom_mode(*native.biosmode.effective()),
            source.biosmode_was_explicit(),
            native.biosmode.was_present(),
        ),
        (
            "samplemode",
            source.samplemode(),
            sample_mode(*native.samplemode.effective()),
            source.samplemode_was_explicit(),
            native.samplemode.was_present(),
        ),
        (
            "lockrommode",
            source.lockrommode(),
            yes_no(*native.lockrommode.effective()),
            source.lockrommode_was_explicit(),
            native.lockrommode.was_present(),
        ),
        (
            "lockbiosmode",
            source.lockbiosmode(),
            yes_no(*native.lockbiosmode.effective()),
            source.lockbiosmode_was_explicit(),
            native.lockbiosmode.was_present(),
        ),
        (
            "locksamplemode",
            source.locksamplemode(),
            yes_no(*native.locksamplemode.effective()),
            source.locksamplemode_was_explicit(),
            native.locksamplemode.was_present(),
        ),
    ] {
        equal(
            &format!("header.romcenter.{name}"),
            &source_value,
            &native_value,
        )?;
        equal(
            &format!("header.romcenter.{name}_presence"),
            &source_present,
            &native_present,
        )?;
    }
    equal(
        "header.romcenter.location",
        &source.location(),
        &native.location,
    )?;
    compare_positions(
        "header.romcenter.attributes",
        source.attribute_positions(),
        &native.attribute_positions,
    )
}

fn compare_option<T>(
    name: &str,
    source_effective: &str,
    native: &mame_coalesce::catalog_logiqx::LogiqxOptionValue<T>,
    source_present: bool,
    render: impl FnOnce(&T) -> &'static str,
) -> VerifyResult {
    equal(
        &format!("clrmamepro.{name}"),
        &source_effective,
        &render(native.effective()),
    )?;
    equal(
        &format!("clrmamepro.{name}_presence"),
        &source_present,
        &native.was_present(),
    )
}

fn compare_game(
    located: &LocatedGame,
    native: &LogiqxGame,
    list_order: usize,
    page_files: &HashMap<OccurrenceId, CatalogFileOccurrence>,
) -> VerifyResult {
    let source = &located.game;
    equal(
        "game.list_order",
        &i64::try_from(list_order)?,
        &native.list_order,
    )?;
    equal("game.location", &located.location, &native.location)?;
    equal("game.name", &source.name(), &native.name.as_str())?;
    equal(
        "game.sourcefile",
        &source.sourcefile_opt(),
        &native.sourcefile.as_deref(),
    )?;
    equal(
        "game.isbios",
        &source.isbios_effective(),
        &yes_no(native.is_bios),
    )?;
    equal(
        "game.isbios_presence",
        &source.isbios_was_explicit(),
        &native.is_bios_was_present,
    )?;
    equal(
        "game.cloneof",
        &source.cloneof(),
        &native.cloneof.as_ref().map(|v| v.target_name.as_str()),
    )?;
    equal(
        "game.romof",
        &source.romof_opt(),
        &native.romof.as_ref().map(|v| v.target_name.as_str()),
    )?;
    equal(
        "game.sampleof",
        &source.sampleof_opt(),
        &native.sampleof.as_ref().map(|v| v.target_name.as_str()),
    )?;
    equal("game.board", &source.board_opt(), &native.board.as_deref())?;
    equal(
        "game.rebuildto",
        &source.rebuildto_opt(),
        &native.rebuildto.as_deref(),
    )?;
    for (name, left, right) in [
        (
            "description",
            source.description_opt(),
            native.description.as_ref().map(|v| v.value.as_str()),
        ),
        (
            "year",
            source.year_opt(),
            native.year.as_ref().map(|v| v.value.as_str()),
        ),
        (
            "manufacturer",
            source.manufacturer_opt(),
            native.manufacturer.as_ref().map(|v| v.value.as_str()),
        ),
    ] {
        equal(&format!("game.{name}"), &left, &right)?;
    }
    compare_positions(
        "game.attributes",
        source.attribute_positions(),
        &native.attribute_positions,
    )?;
    compare_game_text_positions(source, native)?;
    compare_comments(source, &native.comments)?;
    compare_releases(source, &native.releases)?;
    compare_bios_sets(source, &native.bios_sets)?;
    compare_archives(source.archives(), &native.archives)?;
    compare_device_references(located, source, native)?;
    compare_mixed_children(located, native)?;
    compare_media(source, native, page_files)
}

fn compare_game_text_positions(source: &Game, native: &LogiqxGame) -> VerifyResult {
    let source_positions = source
        .text_positions()
        .iter()
        .map(|p| {
            Ok((
                p.field,
                i64::try_from(p.source_order)?,
                p.location.line,
                p.location.column,
            ))
        })
        .collect::<VerifyResult<Vec<_>>>()?;
    let mut native_positions = [
        (GameTextField::Description, native.description.as_ref()),
        (GameTextField::Year, native.year.as_ref()),
        (GameTextField::Manufacturer, native.manufacturer.as_ref()),
    ]
    .into_iter()
    .filter_map(|(field, value)| {
        value.map(|v| (field, v.source_order, v.location.line, v.location.column))
    })
    .collect::<Vec<_>>();
    native_positions.sort_by_key(|position| position.1);
    equal("game.text_positions", &source_positions, &native_positions)
}

fn compare_comments(game: &Game, native: &[LogiqxComment]) -> VerifyResult {
    let source = game.comments();
    equal("game.comments.count", &source.len(), &native.len())?;
    for (index, (left, right)) in source.iter().zip(native).enumerate() {
        equal(
            &format!("game.comments[{index}].text"),
            &left.text(),
            &right.text.as_str(),
        )?;
        equal(
            &format!("game.comments[{index}].location"),
            &left.location(),
            &right.location,
        )?;
        let order = game
            .child_source_order(left.location())
            .ok_or("missing comment source order")?;
        equal(
            &format!("game.comments[{index}].source_order"),
            &i64::try_from(order)?,
            &right.source_order,
        )?;
    }
    Ok(())
}

fn compare_releases(game: &Game, native: &[LogiqxRelease]) -> VerifyResult {
    let source = game.releases();
    equal("game.releases.count", &source.len(), &native.len())?;
    for (index, (left, right)) in source.iter().zip(native).enumerate() {
        let prefix = format!("game.releases[{index}]");
        equal(
            &format!("{prefix}.order"),
            &i64::try_from(index)?,
            &right.release_order,
        )?;
        for (name, a, b) in [
            ("name", left.name(), right.name.as_str()),
            ("region", left.region(), right.region.as_str()),
        ] {
            equal(&format!("{prefix}.{name}"), &a, &b)?;
        }
        equal(
            &format!("{prefix}.language"),
            &left.language(),
            &right.language.as_deref(),
        )?;
        equal(
            &format!("{prefix}.date"),
            &left.date(),
            &right.date.as_deref(),
        )?;
        equal(
            &format!("{prefix}.default"),
            &left.default(),
            &yes_no(right.is_default),
        )?;
        equal(
            &format!("{prefix}.default_presence"),
            &left.default_was_explicit(),
            &right.default_was_present,
        )?;
        let order = game
            .child_source_order(left.location())
            .ok_or("missing release source order")?;
        equal(
            &format!("{prefix}.source_order"),
            &i64::try_from(order)?,
            &right.source_order,
        )?;
        equal(
            &format!("{prefix}.location"),
            &left.location(),
            &right.location,
        )?;
        compare_positions(
            &format!("{prefix}.attributes"),
            left.attribute_positions(),
            &right.attribute_positions,
        )?;
    }
    Ok(())
}

fn compare_bios_sets(game: &Game, native: &[LogiqxBiosSet]) -> VerifyResult {
    let source = game.bios_sets();
    equal("game.bios_sets.count", &source.len(), &native.len())?;
    for (index, (left, right)) in source.iter().zip(native).enumerate() {
        let prefix = format!("game.bios_sets[{index}]");
        equal(
            &format!("{prefix}.order"),
            &i64::try_from(index)?,
            &right.bios_order,
        )?;
        equal(
            &format!("{prefix}.name"),
            &left.name(),
            &right.name.as_str(),
        )?;
        equal(
            &format!("{prefix}.description"),
            &left.description(),
            &right.description.as_str(),
        )?;
        equal(
            &format!("{prefix}.default"),
            &left.default(),
            &yes_no(right.is_default),
        )?;
        equal(
            &format!("{prefix}.default_presence"),
            &left.default_was_explicit(),
            &right.default_was_present,
        )?;
        let order = game
            .child_source_order(left.location())
            .ok_or("missing BIOS-set source order")?;
        equal(
            &format!("{prefix}.source_order"),
            &i64::try_from(order)?,
            &right.source_order,
        )?;
        equal(
            &format!("{prefix}.location"),
            &left.location(),
            &right.location,
        )?;
        compare_positions(
            &format!("{prefix}.attributes"),
            left.attribute_positions(),
            &right.attribute_positions,
        )?;
    }
    Ok(())
}

fn compare_archives(
    source: &[logiqx::Archive],
    native: &[mame_coalesce::catalog_logiqx::LogiqxArchiveReference],
) -> VerifyResult {
    equal("game.archives.count", &source.len(), &native.len())?;
    for (index, (left, right)) in source.iter().zip(native).enumerate() {
        equal(
            &format!("game.archives[{index}].order"),
            &i64::try_from(index)?,
            &right.archive_order,
        )?;
        equal(
            &format!("game.archives[{index}].name"),
            &left.name(),
            &right.name.as_str(),
        )?;
        equal(
            &format!("game.archives[{index}].location"),
            &left.location(),
            &right.location,
        )?;
        compare_positions(
            &format!("game.archives[{index}].attributes"),
            left.attribute_positions(),
            &right.attribute_positions,
        )?;
    }
    Ok(())
}

fn compare_device_references(
    located: &LocatedGame,
    source: &Game,
    native: &LogiqxGame,
) -> VerifyResult {
    let refs = source.device_references();
    equal(
        "game.device_references.count",
        &refs.len(),
        &native.device_references.len(),
    )?;
    equal(
        "game.device_reference_locations.count",
        &located.device_ref_locations.len(),
        &refs.len(),
    )?;
    for (index, ((source_ref, native_ref), location)) in refs
        .iter()
        .zip(&native.device_references)
        .zip(&located.device_ref_locations)
        .enumerate()
    {
        equal(
            &format!("game.device_references[{index}].order"),
            &i64::try_from(index)?,
            &native_ref.reference_order,
        )?;
        equal(
            &format!("game.device_references[{index}].name"),
            &source_ref.name(),
            &native_ref.name.as_str(),
        )?;
        equal(
            &format!("game.device_references[{index}].location"),
            location,
            &native_ref.location,
        )?;
        let source_order = source
            .child_source_order(*location)
            .ok_or("missing device reference child order")?;
        equal(
            &format!("game.device_references[{index}].source_order"),
            &i64::try_from(source_order)?,
            &native_ref.source_order,
        )?;
        compare_positions(
            &format!("game.device_references[{index}].attributes"),
            source_ref.attribute_positions(),
            &native_ref.attribute_positions,
        )?;
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct ChildWitness {
    source_order: i64,
    kind: GameChild,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GameChild {
    Text(GameTextField),
    Comment,
    Release,
    BiosSet,
    Rom,
    Disk,
    Sample,
    Archive,
    DeviceReference,
}

fn compare_mixed_children(located: &LocatedGame, native: &LogiqxGame) -> VerifyResult {
    let source = &located.game;
    let mut expected = Vec::new();
    for position in source.text_positions() {
        expected.push(ChildWitness {
            source_order: i64::try_from(position.source_order)?,
            kind: GameChild::Text(position.field),
        });
    }
    for comment in source.comments() {
        push_child(
            &mut expected,
            source,
            comment.location(),
            GameChild::Comment,
        )?;
    }
    for release in source.releases() {
        push_child(
            &mut expected,
            source,
            release.location(),
            GameChild::Release,
        )?;
    }
    for bios in source.bios_sets() {
        push_child(&mut expected, source, bios.location(), GameChild::BiosSet)?;
    }
    for rom in source.roms() {
        push_child(&mut expected, source, rom.location(), GameChild::Rom)?;
    }
    for disk in source.disks() {
        push_child(&mut expected, source, disk.location(), GameChild::Disk)?;
    }
    for sample in source.samples() {
        push_child(&mut expected, source, sample.location(), GameChild::Sample)?;
    }
    for archive in source.archives() {
        push_child(
            &mut expected,
            source,
            archive.location(),
            GameChild::Archive,
        )?;
    }
    for location in &located.device_ref_locations {
        push_child(&mut expected, source, *location, GameChild::DeviceReference)?;
    }
    expected.sort_by_key(|child| child.source_order);

    let mut actual = Vec::new();
    actual.extend(native.description.iter().map(|v| ChildWitness {
        source_order: v.source_order,
        kind: GameChild::Text(GameTextField::Description),
    }));
    actual.extend(native.year.iter().map(|v| ChildWitness {
        source_order: v.source_order,
        kind: GameChild::Text(GameTextField::Year),
    }));
    actual.extend(native.manufacturer.iter().map(|v| ChildWitness {
        source_order: v.source_order,
        kind: GameChild::Text(GameTextField::Manufacturer),
    }));
    actual.extend(native.comments.iter().map(|v| ChildWitness {
        source_order: v.source_order,
        kind: GameChild::Comment,
    }));
    actual.extend(native.releases.iter().map(|v| ChildWitness {
        source_order: v.source_order,
        kind: GameChild::Release,
    }));
    actual.extend(native.bios_sets.iter().map(|v| ChildWitness {
        source_order: v.source_order,
        kind: GameChild::BiosSet,
    }));
    actual.extend(native.media.iter().map(|v| ChildWitness {
        source_order: v.source_order,
        kind: match v.kind {
            mame_coalesce::catalog_logiqx::LogiqxMediaKind::Rom => GameChild::Rom,
            mame_coalesce::catalog_logiqx::LogiqxMediaKind::Disk => GameChild::Disk,
            mame_coalesce::catalog_logiqx::LogiqxMediaKind::Sample => GameChild::Sample,
        },
    }));
    actual.extend(native.archives.iter().map(|v| ChildWitness {
        source_order: v.source_order,
        kind: GameChild::Archive,
    }));
    actual.extend(native.device_references.iter().map(|v| ChildWitness {
        source_order: v.source_order,
        kind: GameChild::DeviceReference,
    }));
    actual.sort_by_key(|child| child.source_order);
    equal("game.mixed_child_order", &expected, &actual)
}

fn push_child(
    out: &mut Vec<ChildWitness>,
    game: &Game,
    location: RecordLocation,
    kind: GameChild,
) -> VerifyResult {
    let source_order = game
        .child_source_order(location)
        .ok_or("source child location has no parent order")?;
    out.push(ChildWitness {
        source_order: i64::try_from(source_order)?,
        kind,
    });
    Ok(())
}

enum SourceMedia<'a> {
    Rom(&'a Rom),
    Disk(&'a Disk),
    Sample(&'a Sample),
}

impl SourceMedia<'_> {
    const fn kind(&self) -> mame_coalesce::catalog_logiqx::LogiqxMediaKind {
        match self {
            Self::Rom(_) => mame_coalesce::catalog_logiqx::LogiqxMediaKind::Rom,
            Self::Disk(_) => mame_coalesce::catalog_logiqx::LogiqxMediaKind::Disk,
            Self::Sample(_) => mame_coalesce::catalog_logiqx::LogiqxMediaKind::Sample,
        }
    }
    const fn location(&self) -> RecordLocation {
        match self {
            Self::Rom(v) => v.location(),
            Self::Disk(v) => v.location(),
            Self::Sample(v) => v.location(),
        }
    }
}

fn compare_media(
    source: &Game,
    native: &LogiqxGame,
    page_files: &HashMap<OccurrenceId, CatalogFileOccurrence>,
) -> VerifyResult {
    let mut source_media = Vec::new();
    for media in source
        .roms()
        .iter()
        .map(SourceMedia::Rom)
        .chain(source.disks().iter().map(SourceMedia::Disk))
        .chain(source.samples().iter().map(SourceMedia::Sample))
    {
        let order = source
            .child_source_order(media.location())
            .ok_or("missing source media order")?;
        source_media.push((order, media));
    }
    source_media.sort_by_key(|(order, _)| *order);
    let source_media = source_media
        .into_iter()
        .map(|(_, media)| media)
        .collect::<Vec<_>>();
    equal("game.media.count", &source_media.len(), &native.media.len())?;
    for (index, (source_media, native_ref)) in source_media.iter().zip(&native.media).enumerate() {
        let prefix = format!("game.media[{index}]");
        equal(
            &format!("{prefix}.kind"),
            &source_media.kind(),
            &native_ref.kind,
        )?;
        equal(
            &format!("{prefix}.location"),
            &source_media.location(),
            &native_ref.location,
        )?;
        let source_order = source
            .child_source_order(source_media.location())
            .ok_or("missing source media order")?;
        equal(
            &format!("{prefix}.source_order"),
            &i64::try_from(source_order)?,
            &native_ref.source_order,
        )?;
        equal(
            &format!("{prefix}.occurrence_order"),
            &i64::try_from(index)?,
            &native_ref.occurrence_order,
        )?;
        let file = page_files
            .get(&native_ref.occurrence_id)
            .ok_or_else(|| format!("no occurrence payload for {:?}", native_ref.occurrence_id))?;
        equal(
            &format!("{prefix}.format"),
            &file.provenance.format.as_str(),
            &"logiqx",
        )?;
        match (source_media, file.logiqx_file.as_ref()) {
            (SourceMedia::Rom(source_rom), Some(LogiqxFilePayload::Rom(payload))) => {
                compare_rom(source_rom, source, payload, file, &prefix)?;
            }
            (SourceMedia::Disk(source_disk), Some(LogiqxFilePayload::Disk(payload))) => {
                compare_disk(source_disk, source, payload, file, &prefix)?;
            }
            (SourceMedia::Sample(source_sample), Some(LogiqxFilePayload::Sample(payload))) => {
                compare_sample(source_sample, source, payload, file, &prefix)?;
            }
            _ => return Err(format!("{prefix} native payload kind mismatch").into()),
        }
    }
    Ok(())
}

fn compare_rom(
    source: &Rom,
    game: &Game,
    native: &LogiqxRomPayload,
    file: &CatalogFileOccurrence,
    prefix: &str,
) -> VerifyResult {
    equal(
        &format!("{prefix}.name"),
        &Some(source.name()),
        &file.provenance.asset_name.as_deref(),
    )?;
    equal(
        &format!("{prefix}.size_text"),
        &source.size_text(),
        &native.size_text.as_deref(),
    )?;
    for (name, source_text, native_text) in [
        ("crc", source.crc_text(), native.crc_text.as_deref()),
        ("md5", source.md5_text(), native.md5_text.as_deref()),
        ("sha1", source.sha1_text(), native.sha1_text.as_deref()),
    ] {
        equal(&format!("{prefix}.{name}_text"), &source_text, &native_text)?;
    }
    equal(
        &format!("{prefix}.merge"),
        &source.merge(),
        &native.merge_name.as_deref(),
    )?;
    equal(
        &format!("{prefix}.status"),
        &source.effective_status(),
        &dump_status(native.status),
    )?;
    equal(
        &format!("{prefix}.status_presence"),
        &source.status_was_explicit(),
        &native.status_was_present,
    )?;
    equal(
        &format!("{prefix}.date"),
        &source.date(),
        &native.date.as_deref(),
    )?;
    equal(
        &format!("{prefix}.serial"),
        &source.serial(),
        &native.compatibility_serial.as_deref(),
    )?;
    let order = game
        .child_source_order(source.location())
        .ok_or("missing ROM source order")?;
    equal(
        &format!("{prefix}.source_order"),
        &i64::try_from(order)?,
        &native.source_order,
    )?;
    compare_media_positions(
        &format!("{prefix}.attributes"),
        source.attribute_positions(),
        &native.attribute_positions,
    )?;
    digest_verify::compare_declared_digests(
        &format!("{prefix}.declared_digests"),
        [
            (DigestAlgorithm::Crc32, source.crc()),
            (DigestAlgorithm::Md5, source.md5()),
            (DigestAlgorithm::Sha1, source.sha1()),
        ]
        .into_iter()
        .filter_map(|(algorithm, value)| value.map(|value| (algorithm, value, "whole_asset"))),
        &file.digests,
    )
}

fn compare_disk(
    source: &Disk,
    game: &Game,
    native: &LogiqxDiskPayload,
    file: &CatalogFileOccurrence,
    prefix: &str,
) -> VerifyResult {
    equal(
        &format!("{prefix}.name"),
        &Some(source.name()),
        &file.provenance.asset_name.as_deref(),
    )?;
    equal(
        &format!("{prefix}.sha1_text"),
        &source.sha1_text(),
        &native.sha1_text.as_deref(),
    )?;
    equal(
        &format!("{prefix}.md5_text"),
        &source.md5_text(),
        &native.md5_text.as_deref(),
    )?;
    equal(
        &format!("{prefix}.merge"),
        &source.merge(),
        &native.merge_name.as_deref(),
    )?;
    equal(
        &format!("{prefix}.status"),
        &source.effective_status(),
        &dump_status(native.status),
    )?;
    equal(
        &format!("{prefix}.status_presence"),
        &source.status_was_explicit(),
        &native.status_was_present,
    )?;
    let order = game
        .child_source_order(source.location())
        .ok_or("missing disk source order")?;
    equal(
        &format!("{prefix}.source_order"),
        &i64::try_from(order)?,
        &native.source_order,
    )?;
    compare_media_positions(
        &format!("{prefix}.attributes"),
        source.attribute_positions(),
        &native.attribute_positions,
    )?;
    digest_verify::compare_declared_digests(
        &format!("{prefix}.declared_digests"),
        [
            (DigestAlgorithm::Md5, source.md5()),
            (DigestAlgorithm::Sha1, source.sha1()),
        ]
        .into_iter()
        .filter_map(|(algorithm, value)| value.map(|value| (algorithm, value, "disk_data"))),
        &file.digests,
    )
}

fn compare_sample(
    source: &Sample,
    game: &Game,
    native: &LogiqxSamplePayload,
    file: &CatalogFileOccurrence,
    prefix: &str,
) -> VerifyResult {
    equal(
        &format!("{prefix}.name"),
        &Some(source.name()),
        &file.provenance.asset_name.as_deref(),
    )?;
    let order = game
        .child_source_order(source.location())
        .ok_or("missing sample source order")?;
    equal(
        &format!("{prefix}.source_order"),
        &i64::try_from(order)?,
        &native.source_order,
    )?;
    compare_media_positions(
        &format!("{prefix}.attributes"),
        source.attribute_positions(),
        &native.attribute_positions,
    )?;
    digest_verify::compare_declared_digests(
        &format!("{prefix}.declared_digests"),
        [],
        &file.digests,
    )
}

fn compare_positions<F: PartialEq + Debug>(
    field: &str,
    source: &[AttributePosition<F>],
    native: &[AttributePosition<F>],
) -> VerifyResult {
    equal(field, source, native)
}

const fn yes_no(value: LogiqxYesNo) -> &'static str {
    match value {
        LogiqxYesNo::Yes => "yes",
        LogiqxYesNo::No => "no",
    }
}

const fn rom_mode(value: mame_coalesce::catalog_logiqx::LogiqxRomMode) -> &'static str {
    use mame_coalesce::catalog_logiqx::LogiqxRomMode::{Merged, Split, Unmerged};
    match value {
        Split => "split",
        Merged => "merged",
        Unmerged => "unmerged",
    }
}

const fn sample_mode(value: mame_coalesce::catalog_logiqx::LogiqxSampleMode) -> &'static str {
    use mame_coalesce::catalog_logiqx::LogiqxSampleMode::{Merged, Unmerged};
    match value {
        Merged => "merged",
        Unmerged => "unmerged",
    }
}

const fn dump_status(value: LogiqxDumpStatus) -> &'static str {
    match value {
        LogiqxDumpStatus::Good => "good",
        LogiqxDumpStatus::BadDump => "baddump",
        LogiqxDumpStatus::NoDump => "nodump",
        LogiqxDumpStatus::CompatibilityVerified => "verified",
    }
}

#[cfg(test)]
#[path = "logiqx_xml_native_verify/tests.rs"]
mod tests;
