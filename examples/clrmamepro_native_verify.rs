//! Compare a caller-supplied `ClrMamePro` source with its published native facts.

#[path = "clrmamepro_native_verify/compare.rs"]
mod compare;
#[path = "support/digest_verify.rs"]
mod digest_verify;
mod support;

use std::{collections::HashMap, error::Error, fs::File, io::Read, path::Path};

use camino::{Utf8Path, Utf8PathBuf};
use mame_coalesce::{
    catalog_clrmamepro::{self as native, ClrMameProPage, ClrMameProPageLimit, ClrMameProSetChild},
    catalog_files::{CatalogFileOccurrence, occurrences_for_ids},
    clrmamepro::{self as source, Event, EventConsumer},
    database::Database,
    domain::{OccurrenceId, SnapshotKey},
};

use support::catalog_verify::{VerifyResult, equal, open_existing_catalog};

const PAGE_SIZE: usize = 64;
const OCCURRENCE_BATCH_SIZE: usize = 256;

fn main() -> VerifyResult {
    let mut arguments = std::env::args().skip(1);
    let database = Utf8PathBuf::from(arguments.next().ok_or("missing DATABASE")?);
    let snapshot: SnapshotKey = arguments.next().ok_or("missing SNAPSHOT_KEY")?.parse()?;
    let source = arguments.next().ok_or("missing SOURCE_DAT")?;
    if arguments.next().is_some() {
        return Err("expected DATABASE SNAPSHOT_KEY SOURCE_DAT".into());
    }
    verify_path(&database, &snapshot, Path::new(&source))
}

fn verify_path(
    database_path: &Utf8Path,
    snapshot: &SnapshotKey,
    source_path: &Path,
) -> VerifyResult {
    let database = open_existing_catalog(database_path)?;
    let mut bytes = Vec::new();
    File::open(source_path)?.read_to_end(&mut bytes)?;
    let page = native::sets_for_snapshot(
        &database,
        snapshot,
        None,
        ClrMameProPageLimit::new(PAGE_SIZE)?,
    )?;
    equal("snapshot.key", snapshot, &page.snapshot.snapshot_key)?;
    equal(
        "snapshot.format",
        "clrmamepro-dat",
        page.snapshot.format.as_str(),
    )?;
    let files = load_media(&database, &page)?;
    let mut verifier = Verifier {
        database: &database,
        snapshot,
        page,
        files,
        index: 0,
        sets: 0,
        media: 0,
        comments: 0,
        header_seen: false,
        extensions: 0,
    };
    let eof = source::read_with(&bytes, &mut verifier)?;
    verifier.finish(&eof)?;
    println!(
        "VERIFIED snapshot={} sets={} entries={} comments={} skipped_extensions={}",
        snapshot.as_str(),
        verifier.sets,
        verifier.media,
        verifier.comments,
        verifier.extensions
    );
    println!(
        "Memory: the original source, complete source form/set, native document comments and complete children remain input-dependent. Native sets use {PAGE_SIZE}-record pages; occurrence reads use {OCCURRENCE_BATCH_SIZE}-ID batches without truncating children."
    );
    println!(
        "This compares supported CMP source facts and normalized declarations, not independent producer grammar or ROM bytes. Vendor extensions are source-only and excluded from field comparisons."
    );
    Ok(())
}

struct Verifier<'db> {
    database: &'db Database,
    snapshot: &'db SnapshotKey,
    page: ClrMameProPage,
    files: HashMap<OccurrenceId, CatalogFileOccurrence>,
    index: usize,
    sets: usize,
    media: usize,
    comments: usize,
    header_seen: bool,
    extensions: usize,
}

impl EventConsumer for Verifier<'_> {
    type Error = Box<dyn Error>;

    fn consume(&mut self, event: Event) -> VerifyResult {
        match event {
            Event::Header(header, extensions) => {
                if self.header_seen {
                    return Err("duplicate source header callback".into());
                }
                let native = self
                    .page
                    .document
                    .header
                    .as_ref()
                    .ok_or("source has an extra header")?;
                compare::header(&header, native, &self.page.snapshot)?;
                self.header_seen = true;
                self.add_extensions(extensions.len())?;
            }
            Event::Comment(comment) => {
                let native = self
                    .page
                    .document
                    .comments
                    .get(self.comments)
                    .ok_or("source has an extra comment")?;
                let prefix = format!("comment[{}]", self.comments);
                equal(
                    &format!("{prefix}.order"),
                    &i64::try_from(self.comments)?,
                    &native.comment_order,
                )?;
                equal(&format!("{prefix}.text"), &comment.text, &native.text)?;
                compare::location(&prefix, comment.location, native.location)?;
                self.comments = self.comments.checked_add(1).ok_or("too many comments")?;
            }
            Event::Set(set) => {
                self.compare_set(&set)?;
                self.add_extensions(set.extensions.len())?;
                for asset in &set.assets {
                    self.add_extensions(asset.extensions.len())?;
                }
            }
            Event::Extension(_) => self.add_extensions(1)?,
        }
        Ok(())
    }
}

impl Verifier<'_> {
    fn add_extensions(&mut self, count: usize) -> VerifyResult {
        self.extensions = self
            .extensions
            .checked_add(count)
            .ok_or("too many extensions")?;
        Ok(())
    }

    fn compare_set(&mut self, set: &source::Set) -> VerifyResult {
        if self.index == self.page.sets.len() {
            equal("page.unmatched_occurrences", &0, &self.files.len())?;
            let cursor = self
                .page
                .next_cursor
                .as_ref()
                .ok_or("source has an extra set")?;
            let next = native::sets_for_snapshot(
                self.database,
                self.snapshot,
                Some(cursor),
                ClrMameProPageLimit::new(PAGE_SIZE)?,
            )?;
            equal("page.snapshot", &self.page.snapshot, &next.snapshot)?;
            equal("page.document", &self.page.document, &next.document)?;
            self.files = load_media(self.database, &next)?;
            self.page = next;
            self.index = 0;
        }
        let native = self
            .page
            .sets
            .get(self.index)
            .ok_or("source has an extra set")?;
        let media = compare::set(set, native, &self.page.snapshot, self.sets, &mut self.files)?;
        self.sets = self.sets.checked_add(1).ok_or("too many sets")?;
        self.media = self
            .media
            .checked_add(media)
            .ok_or("too many media entries")?;
        self.index += 1;
        Ok(())
    }

    fn finish(&self, eof: &source::EofSeal) -> VerifyResult {
        equal(
            "document.header_present",
            &eof.header_present(),
            &self.page.document.header_present,
        )?;
        equal(
            "document.header_callback",
            &eof.header_present(),
            &self.header_seen,
        )?;
        equal("document.set_count", &eof.set_count(), &self.sets)?;
        equal(
            "document.comment_count",
            &i64::try_from(eof.comment_count())?,
            &self.page.document.comment_count,
        )?;
        equal(
            "document.comment_callbacks",
            &eof.comment_count(),
            &self.comments,
        )?;
        equal(
            "document.comment_tail",
            &self.comments,
            &self.page.document.comments.len(),
        )?;
        if !self.header_seen {
            equal("document.header", &None, &self.page.document.header)?;
            equal(
                "snapshot.declared_version",
                &None,
                &self.page.snapshot.declared_version,
            )?;
        }
        equal("page.unmatched_sets", &self.page.sets.len(), &self.index)?;
        equal("page.unmatched_occurrences", &0, &self.files.len())?;
        if self.page.next_cursor.is_some() {
            return Err("unmatched native set page after source EOF".into());
        }
        Ok(())
    }
}

fn load_media(
    database: &Database,
    page: &ClrMameProPage,
) -> VerifyResult<HashMap<OccurrenceId, CatalogFileOccurrence>> {
    let ids = page
        .sets
        .iter()
        .flat_map(|set| &set.children)
        .filter_map(|child| match child {
            ClrMameProSetChild::Rom(rom) => Some(rom.occurrence_id),
            ClrMameProSetChild::Sample(sample) => Some(sample.occurrence_id),
            ClrMameProSetChild::Field { .. } | ClrMameProSetChild::Parent(_) => None,
        })
        .collect::<Vec<_>>();
    let mut files = HashMap::new();
    for batch in ids.chunks(OCCURRENCE_BATCH_SIZE) {
        let native = occurrences_for_ids(database, batch)?;
        equal("page.requested_media_count", &batch.len(), &native.len())?;
        for file in native {
            if !batch.contains(&file.occurrence_id) {
                return Err("unrequested occurrence returned".into());
            }
            if files.insert(file.occurrence_id, file).is_some() {
                return Err("duplicate occurrence in native page".into());
            }
        }
    }
    Ok(files)
}

#[cfg(test)]
#[path = "clrmamepro_native_verify/tests.rs"]
mod tests;
