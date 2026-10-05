//! Verify one caller-supplied software-list original against native query data.
//! Unknown vendor extensions are reported, not compared as catalog fields.

#[path = "mame_softwarelist_native_verify/compare.rs"]
mod compare;
#[path = "support/digest_verify.rs"]
mod digest_verify;
mod support;
#[path = "support/xml_verify.rs"]
mod xml_verify;

use std::{collections::HashMap, error::Error, fs::File, io::Read, path::Path};

use camino::{Utf8Path, Utf8PathBuf};
use mame_coalesce::{
    catalog_files::{CatalogFileOccurrence, occurrences_for_ids},
    catalog_software::{
        self, SoftwareEnvelope, SoftwareListPage, SoftwarePageLimit, SoftwareTitlePage,
    },
    database::Database,
    domain::{OccurrenceId, SnapshotKey},
    mame_softwarelist::{
        self as source, SoftwareDocumentHeader, SoftwareListHeader, SoftwareListMetadata,
    },
};

use support::catalog_verify::{VerifyResult, equal, open_existing_catalog};
use xml_verify::compare_media_positions;

const PAGE_SIZE: usize = 64;
const OCCURRENCE_BATCH_SIZE: usize = 256;

fn main() -> VerifyResult {
    let mut args = std::env::args().skip(1);
    let database_path = Utf8PathBuf::from(args.next().ok_or("missing DATABASE")?);
    let snapshot: SnapshotKey = args.next().ok_or("missing SNAPSHOT_KEY")?.parse()?;
    let source_path = args.next().ok_or("missing SOURCE_XML")?;
    if args.next().is_some() {
        return Err("expected DATABASE SNAPSHOT_KEY SOURCE_XML".into());
    }
    verify_path(&database_path, &snapshot, Path::new(&source_path))
}

fn verify_path(
    database_path: &Utf8Path,
    snapshot: &SnapshotKey,
    source_path: &Path,
) -> VerifyResult {
    let database = open_existing_catalog(database_path)?;
    let mut source_bytes = Vec::new();
    File::open(source_path)?.read_to_end(&mut source_bytes)?;
    let page = catalog_software::lists_for_snapshot(
        &database,
        snapshot,
        SoftwarePageLimit::new(PAGE_SIZE)?,
        None,
    )?;
    equal("snapshot.key", snapshot, &page.snapshot.snapshot_key)?;
    equal(
        "snapshot.format",
        "mame-softwarelist-xml",
        page.snapshot.format.as_str(),
    )?;
    let validated = source::read_with::<_, Box<dyn Error>>(
        &source_bytes,
        |header| Verifier::new(&database, snapshot, page, &header),
        |verifier, list| verifier.start_list(&list),
        |verifier, item| verifier.item(&item),
        |verifier, metadata| verifier.end_list(&metadata),
        |verifier, _extension| {
            verifier.extensions = verifier
                .extensions
                .checked_add(1)
                .ok_or("too many extensions")?;
            Ok(())
        },
    )?;
    let verifier = validated.into_inner();
    verifier.finish()?;
    println!(
        "VERIFIED snapshot={} lists={} titles={} entries={} skipped_extensions={}",
        snapshot.as_str(),
        verifier.lists,
        verifier.titles,
        verifier.entries,
        verifier.extensions
    );
    println!(
        "Memory: original/decoded buffers and coordinates remain input-dependent, plus one complete source item, one {PAGE_SIZE}-list page and one {PAGE_SIZE}-title page with complete children. Occurrence requests use {OCCURRENCE_BATCH_SIZE}-ID batches; no child cap is imposed."
    );
    println!(
        "This verifies native source facts and qualified digest assertions, not strict producer grammar or loader execution. Vendor extensions are source-only and excluded from field comparisons."
    );
    Ok(())
}

struct Verifier<'db> {
    database: &'db Database,
    snapshot: &'db SnapshotKey,
    page: SoftwareListPage,
    list_index: usize,
    current: Option<CurrentList>,
    lists: usize,
    titles: usize,
    entries: usize,
    extensions: usize,
}

struct CurrentList {
    page: SoftwareTitlePage,
    files: HashMap<OccurrenceId, CatalogFileOccurrence>,
    index: usize,
    titles: usize,
}

impl<'db> Verifier<'db> {
    fn new(
        database: &'db Database,
        snapshot: &'db SnapshotKey,
        page: SoftwareListPage,
        header: &SoftwareDocumentHeader,
    ) -> VerifyResult<Self> {
        let expected = match header.root_kind {
            source::SoftwareListRootKind::SingleList => SoftwareEnvelope::SingleList,
            source::SoftwareListRootKind::PluralLists => SoftwareEnvelope::PluralLists {
                build: header.build.clone(),
            },
        };
        equal("document.envelope", &expected, &page.snapshot.envelope)?;
        compare_media_positions(
            "document.attributes",
            &header.attribute_positions,
            &page.snapshot.wrapper_attribute_positions,
        )?;
        Ok(Self {
            database,
            snapshot,
            page,
            list_index: 0,
            current: None,
            lists: 0,
            titles: 0,
            entries: 0,
            extensions: 0,
        })
    }

    fn start_list(&mut self, header: &SoftwareListHeader) -> VerifyResult {
        if self.current.is_some() {
            return Err("nested software-list callback".into());
        }
        if self.list_index == self.page.lists.len() {
            let cursor = self
                .page
                .next_cursor
                .as_ref()
                .ok_or("source has extra list")?;
            let next = catalog_software::lists_for_snapshot(
                self.database,
                self.snapshot,
                SoftwarePageLimit::new(PAGE_SIZE)?,
                Some(cursor),
            )?;
            equal("list.page.snapshot", &self.page.snapshot, &next.snapshot)?;
            self.page = next;
            self.list_index = 0;
        }
        let native = self
            .page
            .lists
            .get(self.list_index)
            .ok_or("source has extra list")?;
        equal("list.name", header.name.as_str(), native.name.as_str())?;
        equal("list.description", &header.description, &native.description)?;
        equal("list.order", &i64::try_from(self.lists)?, &native.order)?;
        equal(
            "list.source_order",
            &i64::try_from(header.source_order)?,
            &native.source_order,
        )?;
        equal(
            "list.location",
            &(header.location.line, header.location.column),
            &(native.location.line, native.location.column),
        )?;
        let page = catalog_software::titles_for_list(
            self.database,
            self.snapshot,
            native.id,
            SoftwarePageLimit::new(PAGE_SIZE)?,
            None,
        )?;
        equal("title.page.snapshot", &self.page.snapshot, &page.snapshot)?;
        equal("title.page.list", native, &page.list)?;
        let files = load_media(self.database, &page)?;
        self.current = Some(CurrentList {
            page,
            files,
            index: 0,
            titles: 0,
        });
        Ok(())
    }

    fn item(&mut self, item: &source::SoftwareItem) -> VerifyResult {
        let current = self.current.as_mut().ok_or("software item outside list")?;
        if current.index == current.page.titles.len() {
            equal("title.page.unmatched_entries", &0, &current.files.len())?;
            let cursor = current
                .page
                .next_cursor
                .as_ref()
                .ok_or("source has extra title")?;
            let next = catalog_software::titles_for_list(
                self.database,
                self.snapshot,
                current.page.list.id,
                SoftwarePageLimit::new(PAGE_SIZE)?,
                Some(cursor),
            )?;
            equal(
                "title.page.snapshot",
                &current.page.snapshot,
                &next.snapshot,
            )?;
            equal("title.page.list", &current.page.list, &next.list)?;
            current.files = load_media(self.database, &next)?;
            current.page = next;
            current.index = 0;
        }
        let native = current
            .page
            .titles
            .get(current.index)
            .ok_or("source has extra title")?;
        compare::title(
            item,
            native,
            current.titles,
            &current.page.list,
            &mut current.files,
        )?;
        let entries =
            item.parts
                .iter()
                .flat_map(|part| &part.areas)
                .try_fold(0_usize, |sum, area| {
                    sum.checked_add(area.components.len())
                        .ok_or("too many entries")
                })?;
        self.entries = self
            .entries
            .checked_add(entries)
            .ok_or("too many entries")?;
        self.titles = self.titles.checked_add(1).ok_or("too many titles")?;
        current.titles = current.titles.checked_add(1).ok_or("too many titles")?;
        current.index += 1;
        Ok(())
    }

    fn end_list(&mut self, metadata: &SoftwareListMetadata) -> VerifyResult {
        let current = self.current.take().ok_or("list ended without start")?;
        equal("list.notes", &metadata.notes, &current.page.list.notes)?;
        compare_media_positions(
            "list.attributes",
            &metadata.attribute_positions,
            &current.page.list.attribute_positions,
        )?;
        compare::text_positions(
            "list",
            &metadata.text_positions,
            &current.page.list.text_positions,
        )?;
        equal(
            "list.remaining_titles",
            &current.index,
            &current.page.titles.len(),
        )?;
        equal(
            "list.remaining_title_pages",
            &false,
            &current.page.next_cursor.is_some(),
        )?;
        equal("list.unmatched_entries", &0, &current.files.len())?;
        self.lists = self.lists.checked_add(1).ok_or("too many lists")?;
        self.list_index += 1;
        Ok(())
    }

    fn finish(&self) -> VerifyResult {
        equal("document.open_list", &false, &self.current.is_some())?;
        equal(
            "document.remaining_lists",
            &self.list_index,
            &self.page.lists.len(),
        )?;
        equal(
            "document.remaining_list_pages",
            &false,
            &self.page.next_cursor.is_some(),
        )
    }
}

fn load_media(
    database: &Database,
    page: &SoftwareTitlePage,
) -> VerifyResult<HashMap<OccurrenceId, CatalogFileOccurrence>> {
    let ids = page
        .titles
        .iter()
        .flat_map(|title| &title.parts)
        .flat_map(|part| &part.areas)
        .flat_map(|area| area.entry_ids.iter().copied())
        .collect::<Vec<_>>();
    let mut files = HashMap::with_capacity(ids.len());
    for chunk in ids.chunks(OCCURRENCE_BATCH_SIZE) {
        let occurrences = occurrences_for_ids(database, chunk)?;
        equal("media.batch.count", &chunk.len(), &occurrences.len())?;
        for file in occurrences {
            let provenance = &file.provenance;
            equal(
                "media.snapshot",
                page.snapshot.snapshot_key.as_str(),
                provenance.snapshot_key.as_str(),
            )?;
            equal(
                "media.document",
                page.snapshot.document_key.to_string().as_str(),
                provenance.document_key.as_str(),
            )?;
            equal(
                "media.interpretation",
                page.snapshot.interpretation_key.as_str(),
                provenance.interpretation_key.as_str(),
            )?;
            equal(
                "media.format",
                page.snapshot.format.as_str(),
                provenance.format.as_str(),
            )?;
            equal(
                "media.source",
                page.snapshot.source_key.as_str(),
                provenance.source_key.as_str(),
            )?;
            equal(
                "media.catalog",
                page.snapshot.catalog_key.as_str(),
                provenance.catalog_key.as_str(),
            )?;
            if !chunk.contains(&file.occurrence_id)
                || files.insert(file.occurrence_id, file).is_some()
            {
                return Err("media batch has unrequested or duplicate occurrence".into());
            }
        }
    }
    equal("media.total.count", &ids.len(), &files.len())?;
    Ok(files)
}

#[cfg(test)]
#[path = "mame_softwarelist_native_verify/tests.rs"]
mod tests;
