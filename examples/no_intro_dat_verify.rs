use std::{env, error::Error, fs::File, io::Read, path::Path};

use camino::Utf8PathBuf;
use mame_coalesce::{
    NoIntroDatMode,
    catalog_files::NoIntroDatEvidenceScope,
    catalog_no_intro_dat::{
        self, DeclaredText, NoIntroDatForceNoDump, NoIntroDatGame, NoIntroDatGameChild,
        NoIntroDatHeader, NoIntroDatHeaderChild, NoIntroDatHeaderField, NoIntroDatPage,
        NoIntroDatPageLimit, NoIntroDatRelease, NoIntroDatRomField, NoIntroDatRomReference,
    },
    database::Database,
    domain::SnapshotKey,
    logiqx::{AttributeLocation, AttributePosition},
    no_intro_dat_xml::{
        self, ClrMameProOptions, Document, Game, Header, Release, Rom, RomCenterOptions,
    },
};

mod support;

use support::catalog_verify::{VerifyResult, equal, open_existing_catalog};

const PAGE_SIZE: usize = 64;

fn main() -> Result<(), Box<dyn Error>> {
    run(env::args().skip(1))
}

fn run(arguments: impl IntoIterator<Item = String>) -> VerifyResult {
    let mut arguments = arguments.into_iter();
    let database_argument = arguments.next().ok_or("missing DATABASE")?;
    let snapshot_argument = arguments.next().ok_or("missing SNAPSHOT_KEY")?;
    let source_argument = arguments.next().ok_or("missing SOURCE_PATH")?;
    if arguments.next().is_some() {
        return Err("expected DATABASE SNAPSHOT_KEY SOURCE_PATH".into());
    }

    let database_path = Utf8PathBuf::from(database_argument);
    let database = open_existing_catalog(&database_path)?;
    let snapshot: SnapshotKey = snapshot_argument.parse()?;
    let source_path = Path::new(&source_argument);
    // The public reader streams parsed games through its callback, but currently takes the
    // original bytes as a slice, so retain that byte buffer while keeping parsed state bounded.
    let mut source_bytes = Vec::new();
    File::open(source_path)?.read_to_end(&mut source_bytes)?;

    let page = catalog_no_intro_dat::games_for_snapshot(
        &database,
        &snapshot,
        None,
        NoIntroDatPageLimit::new(PAGE_SIZE)?,
    )?;
    let mode = page.document.mode;
    ensure_page_identity(&page, &snapshot, mode)?;
    let mut verifier = Verifier::new(&database, snapshot.clone(), mode, page);

    let completed = no_intro_dat_xml::read_with::<_, Box<dyn Error>>(
        &source_bytes,
        mode,
        |source_document| {
            verifier.compare_document(&source_document)?;
            Ok(verifier)
        },
        |verifier, source_game| verifier.compare_game(&source_game),
    )?;
    let verifier = completed.into_inner();
    verifier.finish()?;

    println!(
        "VERIFIED snapshot={} mode={} games={} header_children={} game_children={}",
        snapshot.as_str(),
        mode.as_str(),
        verifier.games,
        verifier.header_children,
        verifier.children,
    );
    println!(
        "Scope rule: a present ClrMamePro header filter or ROM header attribute, including empty text, means Unknown; otherwise WholeFile."
    );
    println!(
        "Memory scope: one parsed source game and one {PAGE_SIZE}-game query page; source buffers and coordinate bookkeeping remain input-dependent."
    );
    println!(
        "Not persisted by the native contract: valid hash spelling/case, root QName, and schemaLocation attribute QName/position."
    );
    Ok(())
}

struct Verifier<'db> {
    database: &'db Database,
    snapshot: SnapshotKey,
    mode: NoIntroDatMode,
    page: NoIntroDatPage,
    page_index: usize,
    games: usize,
    header_children: usize,
    children: usize,
    source_has_header_filter: bool,
}

impl<'db> Verifier<'db> {
    const fn new(
        database: &'db Database,
        snapshot: SnapshotKey,
        mode: NoIntroDatMode,
        page: NoIntroDatPage,
    ) -> Self {
        Self {
            database,
            snapshot,
            mode,
            page,
            page_index: 0,
            games: 0,
            header_children: 0,
            children: 0,
            source_has_header_filter: false,
        }
    }

    fn compare_document(&mut self, source: &Document) -> VerifyResult {
        ensure_page_identity(&self.page, &self.snapshot, self.mode)?;
        let catalog_document = &self.page.document;
        equal(
            "document.schema_location",
            &source.schema_location,
            &catalog_document.schema_location,
        )?;
        equal(
            "document.location",
            &source.location,
            &catalog_document.location,
        )?;
        equal("document.mode", &self.mode, &catalog_document.mode)?;
        equal(
            "snapshot.declared_version",
            &source
                .header
                .version
                .as_ref()
                .map(|value| value.value.as_str()),
            &self.page.snapshot.declared_version.as_deref(),
        )?;
        compare_header(&source.header, &catalog_document.header)?;
        self.header_children = catalog_document.header.children.len();
        self.source_has_header_filter = source
            .header
            .clrmamepro
            .as_ref()
            .is_some_and(|directive| directive.header.is_some());
        Ok(())
    }

    fn compare_game(&mut self, source: &Game) -> VerifyResult {
        while self.page_index >= self.page.games.len() {
            let Some(cursor) = self.page.next_cursor.take() else {
                return Err(format!(
                    "extra source game at source list order {}",
                    source.list_order
                )
                .into());
            };
            self.page = catalog_no_intro_dat::games_for_snapshot(
                self.database,
                &self.snapshot,
                Some(&cursor),
                NoIntroDatPageLimit::new(PAGE_SIZE)?,
            )?;
            ensure_page_identity(&self.page, &self.snapshot, self.mode)?;
            self.page_index = 0;
        }

        let game_index = self.games;
        let catalog_game = &self.page.games[self.page_index];
        let child_count = compare_game(
            source,
            catalog_game,
            game_index,
            self.source_has_header_filter,
        )?;
        self.children = self
            .children
            .checked_add(child_count)
            .ok_or("verified child count overflow")?;
        self.games = self
            .games
            .checked_add(1)
            .ok_or("verified game count overflow")?;
        self.page_index += 1;
        Ok(())
    }

    fn finish(&self) -> VerifyResult {
        if self.page_index < self.page.games.len() {
            return Err(format!(
                "missing source game at persisted list order {}",
                self.page.games[self.page_index].list_order
            )
            .into());
        }
        if self.page.next_cursor.is_some() {
            return Err("missing source game(s) after the final source game".into());
        }
        Ok(())
    }
}

fn ensure_page_identity(
    page: &NoIntroDatPage,
    snapshot: &SnapshotKey,
    mode: NoIntroDatMode,
) -> VerifyResult {
    equal(
        "snapshot.key",
        page.snapshot.snapshot_key.as_str(),
        snapshot.as_str(),
    )?;
    equal(
        "snapshot.format",
        page.snapshot.format.as_str(),
        mode.as_str(),
    )?;
    equal("document.mode", &page.document.mode, &mode)
}

fn compare_header(source: &Header, catalog: &NoIntroDatHeader) -> VerifyResult {
    equal(
        "header.source_order",
        &source.source_order,
        &catalog.source_order,
    )?;
    equal("header.location", &source.location, &catalog.location)?;

    for (field, name, value) in [
        (NoIntroDatHeaderField::Id, "id", &source.id),
        (NoIntroDatHeaderField::Name, "name", &source.name),
        (
            NoIntroDatHeaderField::Description,
            "description",
            &source.description,
        ),
        (NoIntroDatHeaderField::Version, "version", &source.version),
        (NoIntroDatHeaderField::Date, "date", &source.date),
        (NoIntroDatHeaderField::Author, "author", &source.author),
        (
            NoIntroDatHeaderField::Homepage,
            "homepage",
            &source.homepage,
        ),
        (NoIntroDatHeaderField::Url, "url", &source.url),
        (
            NoIntroDatHeaderField::Trademarks,
            "trademarks",
            &source.trademarks,
        ),
        (NoIntroDatHeaderField::Piracy, "piracy", &source.piracy),
        (NoIntroDatHeaderField::Subset, "subset", &source.subset),
        (NoIntroDatHeaderField::Comment, "comment", &source.comment),
    ] {
        let persisted = catalog.children.iter().find_map(|child| match child {
            NoIntroDatHeaderChild::Text {
                field: actual,
                value,
            } if *actual == field => Some(value),
            _ => None,
        });
        equal(&format!("header.{name}"), &value.as_ref(), &persisted)?;
    }

    compare_clrmamepro(source.clrmamepro.as_ref(), catalog)?;
    compare_romcenter(source.romcenter.as_ref(), catalog)?;
    let source_order = source_header_order(source);
    let catalog_order = catalog
        .children
        .iter()
        .map(|child| (child.source_order(), header_child_kind(child)))
        .collect::<Vec<_>>();
    equal(
        "header.children order/positions",
        &source_order,
        &catalog_order,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeaderChildKind {
    Text(NoIntroDatHeaderField),
    ClrMamePro,
    RomCenter,
}

fn source_header_order(source: &Header) -> Vec<(usize, HeaderChildKind)> {
    let mut order = Vec::new();
    for (field, value) in [
        (NoIntroDatHeaderField::Id, &source.id),
        (NoIntroDatHeaderField::Name, &source.name),
        (NoIntroDatHeaderField::Description, &source.description),
        (NoIntroDatHeaderField::Version, &source.version),
        (NoIntroDatHeaderField::Date, &source.date),
        (NoIntroDatHeaderField::Author, &source.author),
        (NoIntroDatHeaderField::Homepage, &source.homepage),
        (NoIntroDatHeaderField::Url, &source.url),
        (NoIntroDatHeaderField::Trademarks, &source.trademarks),
        (NoIntroDatHeaderField::Piracy, &source.piracy),
        (NoIntroDatHeaderField::Subset, &source.subset),
        (NoIntroDatHeaderField::Comment, &source.comment),
    ] {
        if let Some(value) = value {
            order.push((value.source_order, HeaderChildKind::Text(field)));
        }
    }
    if let Some(value) = &source.clrmamepro {
        order.push((value.source_order, HeaderChildKind::ClrMamePro));
    }
    if let Some(value) = &source.romcenter {
        order.push((value.source_order, HeaderChildKind::RomCenter));
    }
    order.sort_by_key(|(source_order, _)| *source_order);
    order
}

const fn header_child_kind(child: &NoIntroDatHeaderChild) -> HeaderChildKind {
    match child {
        NoIntroDatHeaderChild::Text { field, .. } => HeaderChildKind::Text(*field),
        NoIntroDatHeaderChild::ClrMamePro(_) => HeaderChildKind::ClrMamePro,
        NoIntroDatHeaderChild::RomCenter(_) => HeaderChildKind::RomCenter,
    }
}

fn compare_clrmamepro(
    source: Option<&ClrMameProOptions>,
    catalog: &NoIntroDatHeader,
) -> VerifyResult {
    let persisted = catalog.children.iter().find_map(|child| match child {
        NoIntroDatHeaderChild::ClrMamePro(value) => Some(value),
        _ => None,
    });
    equal(
        "header.clrmamepro presence",
        &source.is_some(),
        &persisted.is_some(),
    )?;
    let (Some(source), Some(persisted)) = (source, persisted) else {
        return Ok(());
    };
    equal(
        "header.clrmamepro.source_order",
        &source.source_order,
        &persisted.source_order,
    )?;
    equal(
        "header.clrmamepro.location",
        &source.location,
        &persisted.location,
    )?;
    equal(
        "header.clrmamepro.forcenodump",
        &source.forcenodump.as_ref(),
        &persisted.forcenodump.as_ref(),
    )?;
    equal(
        "header.clrmamepro.header",
        &source.header.as_ref(),
        &persisted.header.as_ref(),
    )?;
    equal(
        "header.clrmamepro.forcenodump_effective",
        &effective_forcenodump(source.forcenodump.as_ref()),
        &persisted.forcenodump_effective,
    )
}

fn effective_forcenodump(value: Option<&DeclaredText>) -> Option<NoIntroDatForceNoDump> {
    let token = value.map_or("obsolete", |value| {
        value.value.trim_matches(['\t', '\n', '\r', ' '])
    });
    match token {
        "obsolete" => Some(NoIntroDatForceNoDump::Obsolete),
        "required" => Some(NoIntroDatForceNoDump::Required),
        "ignore" => Some(NoIntroDatForceNoDump::Ignore),
        _ => None,
    }
}

fn compare_romcenter(
    source: Option<&RomCenterOptions>,
    catalog: &NoIntroDatHeader,
) -> VerifyResult {
    let persisted = catalog.children.iter().find_map(|child| match child {
        NoIntroDatHeaderChild::RomCenter(value) => Some(value),
        _ => None,
    });
    equal(
        "header.romcenter presence",
        &source.is_some(),
        &persisted.is_some(),
    )?;
    let (Some(source), Some(persisted)) = (source, persisted) else {
        return Ok(());
    };
    equal(
        "header.romcenter.source_order",
        &source.source_order,
        &persisted.source_order,
    )?;
    equal(
        "header.romcenter.location",
        &source.location,
        &persisted.location,
    )?;
    equal(
        "header.romcenter.plugin",
        &source.plugin.as_ref(),
        &persisted.plugin.as_ref(),
    )
}

fn compare_game(
    source: &Game,
    catalog: &NoIntroDatGame,
    game_index: usize,
    source_has_header_filter: bool,
) -> VerifyResult<usize> {
    let prefix = format!("game[{game_index}]");
    equal(
        &format!("{prefix}.list_order"),
        &source.list_order,
        &usize::try_from(catalog.list_order)?,
    )?;
    equal(
        &format!("{prefix}.source_order"),
        &source.source_order,
        &catalog.source_order,
    )?;
    equal(
        &format!("{prefix}.location"),
        &source.location,
        &catalog.location,
    )?;
    equal(&format!("{prefix}.name"), &source.name, &catalog.name)?;
    equal(
        &format!("{prefix}.publisher_id"),
        &source.id.as_ref(),
        &catalog.publisher_id.as_ref(),
    )?;
    equal(
        &format!("{prefix}.cloneof"),
        &source.cloneof.as_ref(),
        &catalog.cloneof.as_ref().map(|parent| &parent.target),
    )?;
    equal(
        &format!("{prefix}.cloneofid"),
        &source.cloneofid.as_ref(),
        &catalog.cloneofid.as_ref().map(|parent| &parent.target),
    )?;

    // CatalogSetId, relationship IDs, ROM occurrence IDs and release game IDs are generated
    // identities. The XML has no corresponding literals; owner matching is source order only.
    let source_children = source_children(source)?;
    equal(
        &format!("{prefix}.children count"),
        &source_children.len(),
        &catalog.children.len(),
    )?;
    for (child_index, (source_child, catalog_child)) in
        source_children.iter().zip(&catalog.children).enumerate()
    {
        compare_game_child(
            source_child,
            catalog_child,
            &prefix,
            child_index,
            source_has_header_filter,
        )?;
    }
    Ok(source_children.len())
}

#[derive(Debug)]
enum SourceGameChild<'a> {
    Description(&'a DeclaredText),
    Category(i64, &'a DeclaredText),
    Identifier(i64, &'a DeclaredText),
    Release(i64, &'a Release),
    Rom(i64, &'a Rom),
}

impl SourceGameChild<'_> {
    const fn source_order(&self) -> usize {
        match self {
            Self::Description(value) | Self::Category(_, value) | Self::Identifier(_, value) => {
                value.source_order
            }
            Self::Release(_, value) => value.source_order,
            Self::Rom(_, value) => value.source_order,
        }
    }
}

fn source_children(source: &Game) -> VerifyResult<Vec<SourceGameChild<'_>>> {
    let mut children = Vec::with_capacity(
        usize::from(source.description.is_some())
            .checked_add(source.categories.len())
            .and_then(|value| value.checked_add(source.identifiers.len()))
            .and_then(|value| value.checked_add(source.releases.len()))
            .and_then(|value| value.checked_add(source.roms.len()))
            .ok_or("source child count overflow")?,
    );
    if let Some(value) = &source.description {
        children.push(SourceGameChild::Description(value));
    }
    for (order, value) in source.categories.iter().enumerate() {
        children.push(SourceGameChild::Category(i64::try_from(order)?, value));
    }
    for (order, value) in source.identifiers.iter().enumerate() {
        children.push(SourceGameChild::Identifier(i64::try_from(order)?, value));
    }
    for (order, value) in source.releases.iter().enumerate() {
        children.push(SourceGameChild::Release(i64::try_from(order)?, value));
    }
    for (order, value) in source.roms.iter().enumerate() {
        children.push(SourceGameChild::Rom(i64::try_from(order)?, value));
    }
    children.sort_by_key(SourceGameChild::source_order);
    Ok(children)
}

fn compare_game_child(
    source: &SourceGameChild<'_>,
    catalog: &NoIntroDatGameChild,
    game_prefix: &str,
    child_index: usize,
    source_has_header_filter: bool,
) -> VerifyResult {
    let prefix = format!("{game_prefix}.children[{child_index}]");
    match (source, catalog) {
        (SourceGameChild::Description(source), NoIntroDatGameChild::Description(catalog)) => {
            equal(&format!("{game_prefix}.description"), *source, catalog)
        }
        (SourceGameChild::Category(order, source), NoIntroDatGameChild::Category(catalog)) => {
            equal(
                &format!("{prefix}.category_order"),
                order,
                &catalog.family_order,
            )?;
            equal(&format!("{prefix}.category"), *source, &catalog.value)
        }
        (SourceGameChild::Identifier(order, source), NoIntroDatGameChild::Identifier(catalog)) => {
            equal(
                &format!("{prefix}.identifier_order"),
                order,
                &catalog.family_order,
            )?;
            equal(&format!("{prefix}.identifier"), *source, &catalog.value)
        }
        (SourceGameChild::Release(order, source), NoIntroDatGameChild::Release(catalog)) => {
            compare_release(source, catalog, *order, &prefix)
        }
        (SourceGameChild::Rom(order, source), NoIntroDatGameChild::Rom(catalog)) => {
            compare_rom(source, catalog, *order, source_has_header_filter, &prefix)
        }
        _ => Err(
            format!("{prefix} kind/order mismatch: source={source:?}, catalog={catalog:?}").into(),
        ),
    }
}

fn compare_release(
    source: &Release,
    catalog: &NoIntroDatRelease,
    release_order: i64,
    prefix: &str,
) -> VerifyResult {
    equal(
        &format!("{prefix}.release_order"),
        &release_order,
        &catalog.key.release_order(),
    )?;
    equal(
        &format!("{prefix}.release.source_order"),
        &source.source_order,
        &catalog.source_order,
    )?;
    equal(
        &format!("{prefix}.release.location"),
        &source.location,
        &catalog.location,
    )?;
    equal(
        &format!("{prefix}.release.name"),
        &source.name,
        &catalog.name,
    )?;
    equal(
        &format!("{prefix}.release.region"),
        &source.region,
        &catalog.region,
    )
}

fn compare_rom(
    source: &Rom,
    catalog: &NoIntroDatRomReference,
    occurrence_order: i64,
    source_has_header_filter: bool,
    prefix: &str,
) -> VerifyResult {
    let payload = &catalog.payload;
    equal(
        &format!("{prefix}.rom.occurrence_order"),
        &occurrence_order,
        &catalog.occurrence_order,
    )?;
    equal(
        &format!("{prefix}.rom.source_order"),
        &source.source_order,
        &catalog.source_order,
    )?;
    equal(
        &format!("{prefix}.rom.location"),
        &source.location,
        &catalog.location,
    )?;
    equal(
        &format!("{prefix}.rom.name"),
        &source.name.value,
        &payload.name,
    )?;
    equal(
        &format!("{prefix}.rom.size_text"),
        &source.size.as_ref().map(|value| &value.value),
        &payload.size_text.as_ref(),
    )?;
    equal(
        &format!("{prefix}.rom.size"),
        &projected_size(source.size.as_ref()),
        &payload.size,
    )?;
    compare_hash(
        &format!("{prefix}.rom.crc"),
        source.crc.as_ref(),
        payload.crc_text.as_deref(),
        4,
    )?;
    compare_hash(
        &format!("{prefix}.rom.md5"),
        source.md5.as_ref(),
        payload.md5_text.as_deref(),
        16,
    )?;
    compare_hash(
        &format!("{prefix}.rom.sha1"),
        source.sha1.as_ref(),
        payload.sha1_text.as_deref(),
        20,
    )?;
    compare_hash(
        &format!("{prefix}.rom.sha256"),
        source.sha256.as_ref(),
        payload.sha256_text.as_deref(),
        32,
    )?;
    compare_declared_field(
        &format!("{prefix}.rom.status"),
        source.status.as_ref(),
        payload.status_text.as_deref(),
    )?;
    compare_declared_field(
        &format!("{prefix}.rom.serial"),
        source.serial.as_ref(),
        payload.serial_text.as_deref(),
    )?;
    compare_declared_field(
        &format!("{prefix}.rom.header"),
        source.header.as_ref(),
        payload.header_text.as_deref(),
    )?;
    compare_declared_field(
        &format!("{prefix}.rom.date"),
        source.date.as_ref(),
        payload.date_text.as_deref(),
    )?;
    compare_declared_field(
        &format!("{prefix}.rom.mia"),
        source.mia.as_ref(),
        payload.mia_text.as_deref(),
    )?;
    let source_scope = if source_has_header_filter || source.header.is_some() {
        NoIntroDatEvidenceScope::Unknown
    } else {
        NoIntroDatEvidenceScope::WholeFile
    };
    equal(
        &format!("{prefix}.rom.evidence_scope"),
        &source_scope,
        &payload.evidence_scope,
    )?;
    compare_rom_positions(source, catalog, prefix)
}

fn compare_declared_field(
    field: &str,
    source: Option<&DeclaredText>,
    catalog: Option<&str>,
) -> VerifyResult {
    equal(field, &source.map(|value| value.value.as_str()), &catalog)
}

fn compare_hash(
    field: &str,
    source: Option<&DeclaredText>,
    catalog: Option<&str>,
    bytes: usize,
) -> VerifyResult {
    match (source, catalog) {
        (None, None) => Ok(()),
        (Some(source), Some(catalog)) if valid_hex_digest(&source.value, bytes) => {
            if !valid_hex_digest(catalog, bytes) {
                return Err(format!(
                    "{field} mismatch: source has a valid digest, catalog does not"
                )
                .into());
            }
            let source_bytes = hex::decode(&source.value)?;
            let catalog_bytes = hex::decode(catalog)?;
            equal(field, &source_bytes, &catalog_bytes)
        }
        (Some(source), Some(catalog)) => equal(field, source.value.as_str(), catalog),
        _ => equal(field, &source.map(|value| value.value.as_str()), &catalog),
    }
}

fn valid_hex_digest(value: &str, bytes: usize) -> bool {
    value.len() == bytes.saturating_mul(2) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn projected_size(value: Option<&DeclaredText>) -> Option<i64> {
    let raw = value?.value.trim_matches(['\t', '\n', '\r', ' ']);
    let (negative, digits) = match raw.as_bytes().first() {
        Some(b'-') => (true, &raw[1..]),
        Some(b'+') => (false, &raw[1..]),
        _ => (false, raw),
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let significant = digits.trim_start_matches('0');
    if negative && !significant.is_empty() {
        return None;
    }
    if significant.len() > 19 {
        return None;
    }
    let magnitude = if significant.is_empty() {
        0
    } else {
        significant.parse::<i64>().ok()?
    };
    Some(if negative { -magnitude } else { magnitude })
}

fn compare_rom_positions(
    source: &Rom,
    catalog: &NoIntroDatRomReference,
    prefix: &str,
) -> VerifyResult {
    let mut expected = vec![(NoIntroDatRomField::Name, &source.name)];
    expected.extend(
        [
            (NoIntroDatRomField::Size, source.size.as_ref()),
            (NoIntroDatRomField::Crc, source.crc.as_ref()),
            (NoIntroDatRomField::Md5, source.md5.as_ref()),
            (NoIntroDatRomField::Sha1, source.sha1.as_ref()),
            (NoIntroDatRomField::Sha256, source.sha256.as_ref()),
            (NoIntroDatRomField::Status, source.status.as_ref()),
            (NoIntroDatRomField::Serial, source.serial.as_ref()),
            (NoIntroDatRomField::Header, source.header.as_ref()),
            (NoIntroDatRomField::Date, source.date.as_ref()),
            (NoIntroDatRomField::Mia, source.mia.as_ref()),
        ]
        .into_iter()
        .filter_map(|(field, value)| value.map(|value| (field, value))),
    );
    let mut expected = expected
        .into_iter()
        .map(|(field, value)| AttributePosition {
            field,
            source_order: value.source_order,
            location: AttributeLocation {
                line: value.location.line,
                column: value.location.column,
            },
        })
        .collect::<Vec<_>>();
    expected.sort_by_key(|position| position.source_order);
    equal(
        &format!("{prefix}.rom.attribute_positions"),
        &expected,
        &catalog.attribute_positions,
    )
}

#[cfg(test)]
mod tests {
    use std::{error::Error, path::PathBuf};

    use camino::Utf8PathBuf;
    use diesel::{Connection, SqliteConnection, connection::SimpleConnection};
    use mame_coalesce::{
        NoIntroDatMode,
        app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
        database::Database,
        domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
        import_diagnostics::{self, DiagnosticPageLimit},
    };

    use super::run;

    type TestResult<T = ()> = Result<T, Box<dyn Error>>;

    struct Published {
        _directory: tempfile::TempDir,
        database: Utf8PathBuf,
        snapshot: SnapshotKey,
        source: PathBuf,
    }

    impl Published {
        fn new(mode: NoIntroDatMode, stored: &str, source_xml: &str) -> TestResult<Self> {
            let directory = tempfile::tempdir()?;
            let database = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
            let imported = Utf8PathBuf::try_from(directory.path().join("imported.xml"))?;
            let source = directory.path().join("source.xml");
            std::fs::write(&imported, stored)?;
            std::fs::write(&source, source_xml)?;

            let database_handle = Database::open(&database)?;
            let report = app::import_catalog(
                &database_handle,
                &CatalogImportRequest {
                    document_path: imported,
                    format: CatalogDocumentFormat::NoIntroDat(mode),
                    source_key: PublishingSourceKey::new("verify-fixture-source"),
                    source_display_name: "Verifier fixture source".into(),
                    catalog_key: CatalogKey::new("verify-fixture-catalog"),
                    catalog_display_name: "Verifier fixture catalog".into(),
                    scope: CatalogScope::Complete,
                },
            )?;
            if report.status != CatalogImportStatus::Succeeded {
                let diagnostic_page = import_diagnostics::for_run(
                    &database_handle,
                    &report.run_key,
                    None,
                    DiagnosticPageLimit::new(8)?,
                )?;
                return Err(format!(
                    "fixture import failed: status={:?}; summary={:?}; diagnostics={:?}",
                    report.status, diagnostic_page.run.summary, diagnostic_page.diagnostics
                )
                .into());
            }

            Ok(Self {
                _directory: directory,
                database,
                snapshot: report
                    .snapshot_key
                    .ok_or("published fixture has no snapshot")?,
                source,
            })
        }

        fn verify(&self) -> Result<(), Box<dyn Error>> {
            run([
                self.database.as_str().to_owned(),
                self.snapshot.to_string(),
                self.source.to_string_lossy().into_owned(),
            ])
        }
    }

    fn document(games: &str, mode: NoIntroDatMode) -> String {
        document_with_header("", games, mode)
    }

    fn document_with_header(header_children: &str, games: &str, mode: NoIntroDatMode) -> String {
        let author = if matches!(mode, NoIntroDatMode::V3Strict | NoIntroDatMode::V4Strict) {
            "<author>fixture</author>"
        } else {
            ""
        };
        format!(
            "<datafile><header><id>1</id><name>Fixture</name><description>Corpus</description><version>1</version>{author}{header_children}</header>{games}</datafile>"
        )
    }

    fn simple_game(description: &str) -> String {
        format!(
            "<game name='same'><description>{description}</description><rom name='same.bin' size='1' crc='AABBCCDD' md5='00112233445566778899aabbccddeeff' sha1='00112233445566778899aabbccddeeff00112233'/></game>"
        )
    }

    fn ordered_games(count: usize, changed_description: Option<usize>) -> String {
        let mut games = String::new();
        for index in 0..count {
            let description = if changed_description == Some(index) {
                format!("changed-{index}")
            } else {
                format!("game-{index}")
            };
            games.push_str(&format!(
                "<game name='duplicate'><description>{description}</description><rom name='same.bin' crc='AABBCCDD'/></game>"
            ));
        }
        games
    }

    fn digest_game(crc: &str, md5: &str, sha1: &str) -> String {
        format!(
            "<game name='same'><description>same</description><rom name='same.bin' crc='{crc}' md5='{md5}' sha1='{sha1}'/></game>"
        )
    }

    fn invoke(mode: NoIntroDatMode, stored: &str, source: &str) -> TestResult<Published> {
        Published::new(mode, stored, source)
    }

    #[test]
    fn exact_zero_game_documents_match_in_all_four_modes() -> TestResult {
        for mode in [
            NoIntroDatMode::V3Strict,
            NoIntroDatMode::V3Compatible,
            NoIntroDatMode::V4Strict,
            NoIntroDatMode::V4Compatible,
        ] {
            let xml = document("", mode);
            let fixture = invoke(mode, &xml, &xml)?;
            assert!(fixture.verify().is_ok(), "{mode:?} zero-game edition");
        }
        Ok(())
    }

    #[test]
    fn exact_games_match_in_all_four_modes() -> TestResult {
        for mode in [
            NoIntroDatMode::V3Strict,
            NoIntroDatMode::V3Compatible,
            NoIntroDatMode::V4Strict,
            NoIntroDatMode::V4Compatible,
        ] {
            let xml = document(&simple_game("same description"), mode);
            let fixture = invoke(mode, &xml, &xml)?;
            assert!(fixture.verify().is_ok(), "{mode:?} game edition");
        }
        Ok(())
    }

    #[test]
    fn native_header_fields_are_compared_even_when_there_are_no_games() -> TestResult {
        let mode = NoIntroDatMode::V4Compatible;
        let stored = document("", mode);
        let source = stored.replace("<name>Fixture</name>", "<name>Changed</name>");
        let fixture = invoke(mode, &stored, &source)?;
        let error = fixture
            .verify()
            .err()
            .ok_or("header mismatch unexpectedly passed")?;
        assert!(error.to_string().contains("header.name"), "{error}");
        Ok(())
    }

    #[test]
    fn genuine_game_field_mismatch_is_reported() -> TestResult {
        let mode = NoIntroDatMode::V4Compatible;
        let stored = document(&simple_game("stored"), mode);
        let source = document(&simple_game("changed"), mode);
        let fixture = invoke(mode, &stored, &source)?;
        let error = fixture
            .verify()
            .err()
            .ok_or("field mismatch unexpectedly passed")?;
        assert!(error.to_string().contains("game[0].description"), "{error}");
        Ok(())
    }

    #[test]
    fn source_missing_a_persisted_game_fails_at_eof() -> TestResult {
        let mode = NoIntroDatMode::V4Compatible;
        let stored = document(
            &format!("{}{}", simple_game("one"), simple_game("two")),
            mode,
        );
        let source = document(&simple_game("one"), mode);
        let fixture = invoke(mode, &stored, &source)?;
        let error = fixture
            .verify()
            .err()
            .ok_or("missing source game unexpectedly passed")?;
        assert!(error.to_string().contains("missing source game"), "{error}");
        Ok(())
    }

    #[test]
    fn source_with_an_extra_game_fails_without_name_based_matching() -> TestResult {
        let mode = NoIntroDatMode::V4Compatible;
        let stored = document(&simple_game("same"), mode);
        let source = document(
            &format!("{}{}", simple_game("same"), simple_game("same")),
            mode,
        );
        let fixture = invoke(mode, &stored, &source)?;
        let error = fixture
            .verify()
            .err()
            .ok_or("extra source game unexpectedly passed")?;
        assert!(error.to_string().contains("extra source game"), "{error}");
        Ok(())
    }

    #[test]
    fn repeated_and_ordered_children_are_compared_as_values() -> TestResult {
        let mode = NoIntroDatMode::V4Compatible;
        let stored_game = "<game name='repeat'><category>A</category><category>A</category><game_id>0007</game_id><game_id>0007</game_id><description>same</description><rom name='r.bin' size='1'/><release name='R' region='JP'/><release name='R' region='JP'/></game>";
        let reordered_game = "<game name='repeat'><category>A</category><game_id>0007</game_id><category>A</category><game_id>0007</game_id><description>same</description><rom name='r.bin' size='1'/><release name='R' region='JP'/><release name='R' region='JP'/></game>";
        let stored = document(stored_game, mode);
        let source = document(reordered_game, mode);
        let fixture = invoke(mode, &stored, &source)?;
        let error = fixture
            .verify()
            .err()
            .ok_or("changed child order unexpectedly passed")?;
        assert!(error.to_string().contains("game[0].children"), "{error}");
        Ok(())
    }

    #[test]
    fn matching_and_mismatching_games_cross_the_query_page_boundary() -> TestResult {
        let mode = NoIntroDatMode::V4Compatible;
        let stored = document(&ordered_games(66, None), mode);
        let fixture = invoke(mode, &stored, &stored)?;
        assert!(fixture.verify().is_ok(), "matching 66-game source");

        let changed = document(&ordered_games(66, Some(64)), mode);
        std::fs::write(&fixture.source, changed)?;
        let error = fixture
            .verify()
            .err()
            .ok_or("post-boundary game mismatch unexpectedly passed")?;
        assert!(
            error.to_string().contains("game[64].description"),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn malformed_xml_after_the_last_game_fails_instead_of_accepting_prefix() -> TestResult {
        let mode = NoIntroDatMode::V4Compatible;
        let valid = document(&simple_game("complete game"), mode);
        let fixture = invoke(mode, &valid, &valid)?;
        let truncated = valid
            .strip_suffix("</datafile>")
            .ok_or("fixture document has no closing datafile tag")?;
        std::fs::write(&fixture.source, truncated)?;
        assert!(
            fixture.verify().is_err(),
            "truncated source unexpectedly passed"
        );
        Ok(())
    }

    #[test]
    fn empty_global_header_filter_preserves_presence_and_default_force_no_dump() -> TestResult {
        let mode = NoIntroDatMode::V4Compatible;
        let game = "<game name='same'><description>same</description><rom name='same.bin' size='1' crc='AABBCCDD'/></game>";
        let stored = document_with_header("<clrmamepro header='' />", game, mode);
        let fixture = invoke(mode, &stored, &stored)?;
        assert!(fixture.verify().is_ok(), "empty filter/default directive");

        let source_without_filter = document_with_header("<clrmamepro />", game, mode);
        std::fs::write(&fixture.source, source_without_filter)?;
        let error = fixture
            .verify()
            .err()
            .ok_or("missing empty header filter unexpectedly passed")?;
        assert!(
            error.to_string().contains("header.clrmamepro.header"),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn empty_rom_header_is_present_and_selects_unknown_scope() -> TestResult {
        let mode = NoIntroDatMode::V4Compatible;
        let stored_game = "<game name='same'><description>same</description><rom name='same.bin' size='1' crc='AABBCCDD' header=''/></game>";
        let stored = document(stored_game, mode);
        let fixture = invoke(mode, &stored, &stored)?;
        assert!(fixture.verify().is_ok(), "empty ROM header attribute");

        let source_without_header = document(
            "<game name='same'><description>same</description><rom name='same.bin' size='1' crc='AABBCCDD'/></game>",
            mode,
        );
        std::fs::write(&fixture.source, source_without_header)?;
        let error = fixture
            .verify()
            .err()
            .ok_or("missing empty ROM header unexpectedly passed")?;
        assert!(
            error.to_string().contains("game[0].children[1].rom.header"),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn valid_digest_spelling_is_normalized_but_digest_values_still_matter() -> TestResult {
        let mode = NoIntroDatMode::V4Compatible;
        let stored_game = digest_game(
            "AABBCCDD",
            "00112233445566778899AABBCCDDEEFF",
            "00112233445566778899AABBCCDDEEFF00112233",
        );
        let source_game = digest_game(
            "aabbccdd",
            "00112233445566778899aabbccddeeff",
            "00112233445566778899aabbccddeeff00112233",
        );
        let stored = document(&stored_game, mode);
        let source = document(&source_game, mode);
        let fixture = invoke(mode, &stored, &source)?;
        assert!(
            fixture.verify().is_ok(),
            "valid digest hex case normalization"
        );

        let changed_game = digest_game(
            "AABBCCDE",
            "00112233445566778899AABBCCDDEEFF",
            "00112233445566778899AABBCCDDEEFF00112233",
        );
        std::fs::write(&fixture.source, document(&changed_game, mode))?;
        let error = fixture
            .verify()
            .err()
            .ok_or("different valid digest unexpectedly passed")?;
        assert!(
            error.to_string().contains("game[0].children[1].rom.crc"),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn invalid_digest_text_is_compared_exactly() -> TestResult {
        let mode = NoIntroDatMode::V4Compatible;
        let stored_game = "<game name='same'><description>same</description><rom name='same.bin' crc='not-a-crc' md5='00112233445566778899aabbccddeeff' sha1='00112233445566778899aabbccddeeff00112233'/></game>";
        let stored = document(stored_game, mode);
        let fixture = invoke(mode, &stored, &stored)?;
        assert!(fixture.verify().is_ok(), "matching invalid checksum text");

        let changed = document(
            "<game name='same'><description>same</description><rom name='same.bin' crc='NOT-A-CRC' md5='00112233445566778899aabbccddeeff' sha1='00112233445566778899aabbccddeeff00112233'/></game>",
            mode,
        );
        std::fs::write(&fixture.source, changed)?;
        let error = fixture
            .verify()
            .err()
            .ok_or("changed invalid digest text unexpectedly passed")?;
        assert!(
            error.to_string().contains("game[0].children[1].rom.crc"),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn minimum_signed_size_keeps_the_native_null_projection() -> TestResult {
        let mode = NoIntroDatMode::V4Compatible;
        let game = "<game name='same'><description>same</description><rom name='same.bin' size='-9223372036854775808'/></game>";
        let xml = document(game, mode);
        let fixture = invoke(mode, &xml, &xml)?;
        let result = fixture.verify();
        assert!(result.is_ok(), "{result:?}");
        Ok(())
    }

    #[test]
    fn nonzero_sqlite_file_without_catalog_schema_is_not_initialized() -> TestResult {
        let directory = tempfile::tempdir()?;
        let database = Utf8PathBuf::try_from(directory.path().join("empty.sqlite"))?;
        let source = directory.path().join("source.xml");
        let xml = document("", NoIntroDatMode::V4Compatible);
        std::fs::write(&source, xml)?;

        let mut connection = SqliteConnection::establish(database.as_str())?;
        connection.batch_execute("PRAGMA user_version = 1")?;
        drop(connection);
        let before = std::fs::read(&database)?;
        assert!(!before.is_empty(), "fixture must be a nonzero SQLite file");

        let key = format!("sha256:{}", "ab".repeat(32));
        assert!(
            run([
                database.as_str().to_owned(),
                key,
                source.to_string_lossy().into_owned(),
            ])
            .is_err(),
            "non-catalog database unexpectedly accepted"
        );
        let after = std::fs::read(&database)?;
        assert!(
            after == before,
            "rejecting an empty SQLite database must not initialize it (bytes before={}, after={})",
            before.len(),
            after.len()
        );
        Ok(())
    }
}
