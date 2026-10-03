#![allow(clippy::expect_used, clippy::panic)]

use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use diesel::{Connection, connection::InstrumentationEvent};

use crate::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, CatalogSnapshotDiff, PublishingSourceKey, SnapshotKey},
    no_intro_db_xml::NoIntroDatabaseMode,
};
use camino::Utf8PathBuf;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct Catalog {
    directory: tempfile::TempDir,
    database: Database,
    next_document: usize,
}

impl Catalog {
    fn new() -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        Ok(Self {
            database: Database::in_memory()?,
            directory,
            next_document: 0,
        })
    }

    fn import(&mut self, xml: &str) -> TestResult<SnapshotKey> {
        self.import_with_format(
            xml,
            CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
        )
    }

    fn import_with_format(
        &mut self,
        xml: &str,
        format: CatalogDocumentFormat,
    ) -> TestResult<SnapshotKey> {
        let path = Utf8PathBuf::from_path_buf(
            self.directory
                .path()
                .join(format!("{}.xml", self.next_document)),
        )
        .map_err(|_| "non-UTF-8 document path")?;
        self.next_document += 1;
        std::fs::write(&path, xml)?;
        let report = app::import_catalog(
            &self.database,
            &CatalogImportRequest {
                document_path: path,
                format,
                source_key: PublishingSourceKey::new("export-consumers"),
                source_display_name: "Export publisher".into(),
                catalog_key: CatalogKey::new("export-consumers"),
                catalog_display_name: "Export catalog".into(),
                scope: CatalogScope::Complete,
            },
        )?;
        assert_eq!(report.status, CatalogImportStatus::Succeeded, "{xml}");
        Ok(report.snapshot_key.expect("published snapshot"))
    }

    fn diff(&mut self, before: &str, after: &str) -> TestResult<CatalogSnapshotDiff> {
        let before = self.import(before)?;
        let after = self.import(after)?;
        self.diff_snapshots(&before, &after)
    }

    fn diff_snapshots(
        &self,
        before: &SnapshotKey,
        after: &SnapshotKey,
    ) -> TestResult<CatalogSnapshotDiff> {
        Ok(app::diff_catalog_snapshots(&self.database, before, after)?)
    }

    fn root_endpoints(
        &self,
        snapshot: &SnapshotKey,
    ) -> TestResult<Vec<crate::domain::RelationshipEndpoint>> {
        use crate::domain::{
            CatalogRecordKind, CatalogRecordRef, CatalogSetId, RelationshipEndpoint,
        };
        use diesel::{
            QueryableByName, RunQueryDsl, sql_query,
            sql_types::{BigInt, Text},
        };

        #[derive(QueryableByName)]
        struct Owner {
            #[diesel(sql_type = BigInt)]
            set_id: i64,
            #[diesel(sql_type = Text)]
            set_name: String,
        }

        let mut connection = self.database.pool().get()?;
        let owners = sql_query("SELECT sets.set_id, sets.set_name FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING(set_group_id) WHERE groups.snapshot_key=? AND groups.kind='root' ORDER BY sets.list_order")
            .bind::<Text, _>(snapshot.as_str()).load::<Owner>(&mut connection)?;
        Ok(owners
            .into_iter()
            .map(|owner| {
                RelationshipEndpoint::CatalogRecord(CatalogRecordRef::new_owned(
                    snapshot.clone(),
                    CatalogRecordKind::Set,
                    owner.set_name,
                    CatalogSetId::from_database(owner.set_id),
                ))
            })
            .collect())
    }
}

fn document(children: &str) -> String {
    format!("<datafile><game name='same'>{children}</game></datafile>")
}

fn games_document<'a>(games: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    let mut xml = String::from("<datafile>");
    for (name, children) in games {
        write!(xml, "<game name='{name}'>{children}</game>")
            .expect("writing XML fixture to a String cannot fail");
    }
    xml.push_str("</datafile>");
    xml
}

fn assert_named_game_changes<N, C>(catalog: &mut Catalog, cases: &[(N, C, C)]) -> TestResult
where
    N: AsRef<str>,
    C: AsRef<str>,
{
    let before = games_document(
        cases
            .iter()
            .map(|(name, before, _)| (name.as_ref(), before.as_ref())),
    );
    let after = games_document(
        cases
            .iter()
            .map(|(name, _, after)| (name.as_ref(), after.as_ref())),
    );
    let diff = catalog.diff(&before, &after)?;
    assert_eq!(
        diff.records.len(),
        cases.len(),
        "one history record per owner"
    );
    for (name, _, _) in cases {
        let name = name.as_ref();
        let record = diff
            .records
            .iter()
            .find(|record| record.set_name == name)
            .unwrap_or_else(|| panic!("missing history record for {name}"));
        assert!(record.metadata_changed, "{name}");
        assert!(record.requirement_changes.is_empty(), "{name}");
        assert_eq!(
            record.correspondence,
            crate::domain::SnapshotRecordCorrespondence::UniqueName,
            "{name}"
        );
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum NativeOwner {
    Archive,
    SourceDetails,
    SourceSerials,
    ReleaseDetails,
    ReleaseSerials,
    SourceFile,
    ReleaseFile,
}

const ARCHIVE_FIELDS: &[&str] = &[
    "additional",
    "adult",
    "aftermarket",
    "alt",
    "bios",
    "categories",
    "complete",
    "dat",
    "datter_note",
    "description",
    "devstatus",
    "gameid1",
    "gameid2",
    "langchecked",
    "languages",
    "licensed",
    "listed",
    "mergename",
    "name",
    "name_alt",
    "number",
    "physical",
    "region",
    "regparent",
    "showlang",
    "special1",
    "special2",
    "sticky_note",
    "version1",
    "version2",
    "clone",
    "mergeof",
];

const SOURCE_DETAILS_FIELDS: &[&str] = &[
    "comment1",
    "comment2",
    "d_date",
    "d_date_info",
    "dumper",
    "id",
    "link1",
    "link2",
    "link3",
    "media_title",
    "nodump",
    "origin",
    "originalformat",
    "project",
    "r_date",
    "r_date_info",
    "region",
    "rominfo",
    "section",
    "tool",
];

const SOURCE_SERIALS_FIELDS: &[&str] = &[
    "box_barcode",
    "box_serial",
    "chip_serial",
    "digital_serial1",
    "digital_serial2",
    "lockout_serial",
    "media_serial1",
    "media_serial2",
    "media_serial3",
    "mediastamp",
    "pcb_serial",
    "romchip_serial1",
    "romchip_serial2",
    "savechip_serial",
];

const RELEASE_DETAILS_FIELDS: &[&str] = &[
    "archivename",
    "category",
    "comment",
    "date",
    "dirname",
    "group",
    "id",
    "nfo_crc32",
    "nfo_size",
    "nfocrc",
    "nfoname",
    "nfosize",
    "origin",
    "originalformat",
    "region",
    "rominfo",
    "tool",
];

const RELEASE_SERIALS_FIELDS: &[&str] = &[
    "box_barcode",
    "box_serial",
    "media_serial1",
    "mediastamp",
    "pcb_serial",
    "romchip_serial1",
];

const SOURCE_FILE_FIELDS: &[&str] = &[
    "bad",
    "crc32",
    "date",
    "extension",
    "filter",
    "forcename",
    "forcescenename",
    "format",
    "header",
    "id",
    "item",
    "md5",
    "mia",
    "note",
    "origin_sha256",
    "origin_size",
    "serial",
    "sha1",
    "sha256",
    "size",
    "unique",
    "update_type",
    "version",
];

const RELEASE_FILE_FIELDS: &[&str] = &[
    "bad",
    "crc32",
    "extension",
    "forcename",
    "forcescenename",
    "format",
    "header",
    "id",
    "item",
    "md5",
    "note",
    "serial",
    "sha1",
    "sha256",
    "size",
    "update_type",
    "version",
];

impl NativeOwner {
    const ALL: [Self; 7] = [
        Self::Archive,
        Self::SourceDetails,
        Self::SourceSerials,
        Self::ReleaseDetails,
        Self::ReleaseSerials,
        Self::SourceFile,
        Self::ReleaseFile,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Archive => "archive",
            Self::SourceDetails => "source-details",
            Self::SourceSerials => "source-serials",
            Self::ReleaseDetails => "release-details",
            Self::ReleaseSerials => "release-serials",
            Self::SourceFile => "source-file",
            Self::ReleaseFile => "release-file",
        }
    }

    const fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Archive => ARCHIVE_FIELDS,
            Self::SourceDetails => SOURCE_DETAILS_FIELDS,
            Self::SourceSerials => SOURCE_SERIALS_FIELDS,
            Self::ReleaseDetails => RELEASE_DETAILS_FIELDS,
            Self::ReleaseSerials => RELEASE_SERIALS_FIELDS,
            Self::SourceFile => SOURCE_FILE_FIELDS,
            Self::ReleaseFile => RELEASE_FILE_FIELDS,
        }
    }

    fn xml(self, attributes: &str) -> String {
        match self {
            Self::Archive => format!("<archive {attributes}/>"),
            Self::SourceDetails => format!("<source><details {attributes}/></source>"),
            Self::SourceSerials => format!("<source><serials {attributes}/></source>"),
            Self::ReleaseDetails => format!("<release><details {attributes}/></release>"),
            Self::ReleaseSerials => format!("<release><serials {attributes}/></release>"),
            Self::SourceFile => format!("<source><file {attributes}/></source>"),
            Self::ReleaseFile => format!("<release><file {attributes}/></release>"),
        }
    }
}

fn owner_games(groups: &[(NativeOwner, &[&str])], comparison: &str, value: Option<&str>) -> String {
    let mut games = String::new();
    for (owner, fields) in groups {
        for field in *fields {
            let attributes = value
                .map(|value| format!("{field}='{value}'"))
                .unwrap_or_default();
            write!(
                games,
                "<game name='field-{comparison}-{}-{field}'>{}</game>",
                owner.label(),
                owner.xml(&attributes)
            )
            .expect("writing XML fixture to a String cannot fail");
        }
    }
    games
}

fn assert_field_records(diff: &CatalogSnapshotDiff, groups: &[(NativeOwner, &[&str])]) {
    let mut expected = ["value", "presence"]
        .into_iter()
        .flat_map(|comparison| {
            groups.iter().flat_map(move |(owner, fields)| {
                fields
                    .iter()
                    .map(move |field| format!("field-{comparison}-{}-{field}", owner.label()))
            })
        })
        .collect::<Vec<_>>();
    let mut actual = diff
        .records
        .iter()
        .map(|record| record.set_name.clone())
        .collect::<Vec<_>>();
    expected.sort();
    actual.sort();
    assert_eq!(actual, expected, "field game set and count");
}

#[test]
fn history_derives_export_version_and_relationship_provenance_from_native_header() -> TestResult {
    let mut catalog = Catalog::new()?;
    let snapshot = catalog.import("<datafile><header><version>native-v1</version></header><game name='same'><archive number='1' clone='2'/></game></datafile>")?;
    let history =
        app::catalog_snapshot_history(&catalog.database, &CatalogKey::new("export-consumers"))?;
    let entry = history
        .iter()
        .find(|entry| entry.snapshot == snapshot)
        .expect("history entry");
    assert_eq!(entry.declared_version.as_deref(), Some("native-v1"));
    let relationships = app::explain_relationships(&catalog.database)?;
    assert_eq!(relationships.len(), 1);
    assert_eq!(
        relationships[0]
            .source
            .as_ref()
            .expect("source provenance")
            .declared_version
            .as_deref(),
        Some("native-v1")
    );
    Ok(())
}

#[test]
fn repeated_export_versions_are_not_arbitrarily_reduced_to_one_value() -> TestResult {
    let mut catalog = Catalog::new()?;
    let cases = [
        "",
        "<version/>",
        "<version>one</version><version>two</version>",
        "<version>one</version><version>one</version>",
    ];
    let mut snapshots = Vec::with_capacity(cases.len());
    for fields in cases {
        let xml = format!(
            "<datafile><header>{fields}</header><game name='same'><archive clone='2'/></game></datafile>"
        );
        snapshots.push((fields, catalog.import(&xml)?));
    }

    let history =
        app::catalog_snapshot_history(&catalog.database, &CatalogKey::new("export-consumers"))?;
    let explanations = app::explain_relationships(&catalog.database)?;
    for (fields, snapshot) in snapshots {
        let entry = history
            .iter()
            .find(|entry| entry.snapshot == snapshot)
            .expect("history entry");
        assert_eq!(
            entry.declared_version.as_deref(),
            (fields == "<version/>").then_some(""),
            "{fields}"
        );
        let explanation = explanations
            .iter()
            .find(|explanation| {
                matches!(
                    &explanation.claim.subject,
                    crate::domain::RelationshipEndpoint::NoIntroArchive {
                        snapshot: owner, ..
                    } if *owner == snapshot
                )
            })
            .expect("native archive relationship for this snapshot");
        assert_eq!(
            explanation
                .source
                .as_ref()
                .expect("native source provenance")
                .declared_version
                .as_deref(),
            entry.declared_version.as_deref(),
            "relationship provenance must use the same singular version rule: {fields}"
        );
    }
    Ok(())
}

#[test]
fn export_header_order_presence_and_envelope_are_document_metadata() -> TestResult {
    let mut catalog = Catalog::new()?;
    let before = "<datafile><header><version>one</version><author>A</author></header><game name='same'/></datafile>";
    let before_snapshot = catalog.import(before)?;
    for after in [
        "<datafile><header><version>two</version><author>A</author></header><game name='same'/></datafile>",
        "<datafile><header><author>A</author><version>one</version></header><game name='same'/></datafile>",
        "<header><version>one</version><author>A</author></header><datafile><game name='same'/></datafile>",
    ] {
        let after_snapshot = catalog.import(after)?;
        let diff = catalog.diff_snapshots(&before_snapshot, &after_snapshot)?;
        assert!(diff.document_metadata_changed, "{after}");
        assert!(!diff.records[0].metadata_changed);
    }
    let without_header = catalog.import("<datafile><game name='same'/></datafile>")?;
    let with_header = catalog.import("<datafile><header/><game name='same'/></datafile>")?;
    let diff = catalog.diff_snapshots(&without_header, &with_header)?;
    assert!(diff.document_metadata_changed);
    Ok(())
}

#[test]
fn every_native_owner_field_is_visible_to_history() -> TestResult {
    let groups = NativeOwner::ALL.map(|owner| (owner, owner.fields()));
    assert_eq!(
        groups.iter().map(|(_, fields)| fields.len()).sum::<usize>(),
        129,
        "all owner-qualified native field witnesses remain represented"
    );
    let mut catalog = Catalog::new()?;
    // Separate names keep both comparisons independent within one snapshot pair.
    let before = format!(
        "<datafile>{}{}</datafile>",
        owner_games(&groups, "value", Some("before")),
        owner_games(&groups, "presence", None),
    );
    let after = format!(
        "<datafile>{}{}</datafile>",
        owner_games(&groups, "value", Some("after")),
        owner_games(&groups, "presence", Some("")),
    );
    let before_snapshot = catalog.import(&before)?;
    let after_snapshot = catalog.import(&after)?;
    assert_eq!(
        catalog.next_document, 2,
        "one import per comparison document"
    );

    let diff = catalog.diff_snapshots(&before_snapshot, &after_snapshot)?;
    assert_field_records(&diff, &groups);
    assert!(!diff.document_metadata_changed);
    for record in &diff.records {
        assert_eq!(
            record.correspondence,
            crate::domain::SnapshotRecordCorrespondence::UniqueName,
            "field game must have one-to-one history correspondence: {}",
            record.set_name
        );
        assert!(
            record.metadata_changed,
            "native value or declaration presence change missing for {}",
            record.set_name
        );
        assert!(
            record.requirement_changes.is_empty(),
            "unknown-scope declaration became requirement: {}",
            record.set_name
        );
    }
    Ok(())
}

#[test]
fn native_owner_order_and_empty_elements_are_preserved() -> TestResult {
    let mut catalog = Catalog::new()?;
    let cases = [
        (
            "archive-source-order",
            "<archive name='A'/><source/>",
            "<source/><archive name='A'/>",
        ),
        (
            "source-release-order",
            "<source/><release/>",
            "<release/><source/>",
        ),
        (
            "empty-source-details",
            "<source/>",
            "<source><details/></source>",
        ),
        (
            "empty-release-serials",
            "<release/>",
            "<release><serials/></release>",
        ),
        (
            "source-file-order",
            "<source><file id='1'/><file id='2'/></source>",
            "<source><file id='2'/><file id='1'/></source>",
        ),
        (
            "source-file-details-order",
            "<source><file id='1'/><details id='D'/></source>",
            "<source><details id='D'/><file id='1'/></source>",
        ),
        (
            "source-serials-details-order",
            "<source><serials/><details/></source>",
            "<source><details/><serials/></source>",
        ),
        (
            "release-serials-file-order",
            "<release><serials/><file id='1'/></release>",
            "<release><file id='1'/><serials/></release>",
        ),
        (
            "archive-sibling-order",
            "<archive name='A'/><archive name='B'/>",
            "<archive name='B'/><archive name='A'/>",
        ),
    ];
    assert_named_game_changes(&mut catalog, &cases)?;
    Ok(())
}

#[test]
fn formatting_and_physical_locations_do_not_change_native_facts() -> TestResult {
    let mut catalog = Catalog::new()?;
    let before = document(
        "<archive name='A' number='001'/><source><details id='1' dumper='D'/><file id='x' size='0007'/></source>",
    );
    let after = "<datafile>\n<game name='same'>\n<archive name='A' number='001'/>\n<source><details id='1' dumper='D'/><file id='x' size='0007'/></source>\n</game></datafile>";
    let diff = catalog.diff(&before, after)?;
    assert!(!diff.document_metadata_changed);
    assert!(!diff.records[0].metadata_changed);
    assert!(diff.records[0].requirement_changes.is_empty());
    Ok(())
}

#[test]
fn every_header_field_and_repeated_declaration_is_compared() -> TestResult {
    let mut catalog = Catalog::new()?;
    let fields = ["author", "piracy", "trademarks", "url", "version"];
    let mut before_header = String::new();
    for field in fields {
        write!(before_header, "<{field}>A</{field}>")
            .expect("writing XML fixture to a String cannot fail");
    }
    let before = catalog.import(&format!(
        "<datafile><header>{before_header}</header><game name='same'/></datafile>"
    ))?;
    for field in fields {
        for values in [
            format!("<{field}>B</{field}>"),
            format!("<{field}>A</{field}><{field}>A</{field}>"),
        ] {
            let after_header = fields
                .iter()
                .map(|other| {
                    if *other == field {
                        values.clone()
                    } else {
                        format!("<{other}>A</{other}>")
                    }
                })
                .collect::<String>();
            let after = catalog.import(&format!(
                "<datafile><header>{after_header}</header><game name='same'/></datafile>"
            ))?;
            let diff = catalog.diff_snapshots(&before, &after)?;
            assert!(diff.document_metadata_changed, "{field}");
        }
    }
    Ok(())
}

#[test]
fn recognized_attribute_order_is_visible_in_each_native_owner() -> TestResult {
    let mut catalog = Catalog::new()?;
    let cases = [
        (
            "archive-attributes",
            "<archive name='A' number='1'/>",
            "<archive number='1' name='A'/>",
        ),
        (
            "source-details-attributes",
            "<source><details id='1' dumper='D'/></source>",
            "<source><details dumper='D' id='1'/></source>",
        ),
        (
            "source-serials-attributes",
            "<source><serials box_serial='A' pcb_serial='B'/></source>",
            "<source><serials pcb_serial='B' box_serial='A'/></source>",
        ),
        (
            "source-file-attributes",
            "<source><file id='1' note='A'/></source>",
            "<source><file note='A' id='1'/></source>",
        ),
        (
            "release-details-attributes",
            "<release><details id='1' comment='A'/></release>",
            "<release><details comment='A' id='1'/></release>",
        ),
        (
            "release-serials-attributes",
            "<release><serials box_serial='A' pcb_serial='B'/></release>",
            "<release><serials pcb_serial='B' box_serial='A'/></release>",
        ),
        (
            "release-file-attributes",
            "<release><file id='1' note='A'/></release>",
            "<release><file note='A' id='1'/></release>",
        ),
    ];
    assert_named_game_changes(&mut catalog, &cases)?;
    Ok(())
}

#[test]
fn valid_file_origin_and_nfo_digest_changes_do_not_invent_whole_file_requirements() -> TestResult {
    let mut catalog = Catalog::new()?;
    let digest_fields = [
        ("crc32", 8),
        ("md5", 32),
        ("sha1", 40),
        ("sha256", 64),
        ("origin_sha256", 64),
    ];
    let nfo_fields = ["nfocrc", "nfo_crc32"];
    let mut cases = digest_fields
        .iter()
        .map(|(field, width)| {
            (
                (*field).to_owned(),
                format!("<source><file {field}='{}'/></source>", "1".repeat(*width)),
                format!("<source><file {field}='{}'/></source>", "2".repeat(*width)),
            )
        })
        .collect::<Vec<_>>();
    cases.extend(nfo_fields.iter().map(|field| {
        (
            (*field).to_owned(),
            format!("<release><details {field}='11111111'/></release>"),
            format!("<release><details {field}='22222222'/></release>"),
        )
    }));
    assert_named_game_changes(&mut catalog, &cases)?;
    Ok(())
}

#[test]
fn normalized_valid_digest_case_does_not_change_catalog_facts() -> TestResult {
    let mut catalog = Catalog::new()?;
    let before = document(&format!(
        "<source><file sha1='{}' origin_sha256='{}'/></source><release><details nfocrc='abcdef01'/></release>",
        "a".repeat(40),
        "b".repeat(64)
    ));
    let after = document(&format!(
        "<source><file sha1='{}' origin_sha256='{}'/></source><release><details nfocrc='ABCDEF01'/></release>",
        "A".repeat(40),
        "B".repeat(64)
    ));
    let diff = catalog.diff(&before, &after)?;
    assert!(!diff.records[0].metadata_changed);
    assert!(diff.records[0].requirement_changes.is_empty());
    Ok(())
}

#[test]
fn file_claims_remain_attached_to_the_actual_history_owner() -> TestResult {
    let mut catalog = Catalog::new()?;
    let before = document(
        "<source><details id='duplicate' dumper='A'/><file id='duplicate' note='first'/></source><source><details id='duplicate' dumper='B'/><file id='duplicate' note='second'/></source>",
    );
    let after = document(
        "<source><details id='duplicate' dumper='A'/><file id='duplicate' note='second'/></source><source><details id='duplicate' dumper='B'/><file id='duplicate' note='first'/></source>",
    );
    let diff = catalog.diff(&before, &after)?;
    assert!(diff.records[0].metadata_changed);
    assert!(diff.records[0].requirement_changes.is_empty());
    Ok(())
}

#[test]
fn clone_marker_and_reference_and_distinct_nfo_aliases_do_not_collapse() -> TestResult {
    let mut catalog = Catalog::new()?;
    let cases = [
        (
            "clone-marker",
            "<archive clone='P'/>",
            "<archive clone='1'/>",
        ),
        (
            "clone-reference-kind",
            "<archive clone='1'/>",
            "<archive mergeof='1'/>",
        ),
        (
            "nfo-alias",
            "<release><details nfocrc='11111111'/></release>",
            "<release><details nfo_crc32='11111111'/></release>",
        ),
        (
            "file-digest-kind",
            "<source><file sha256='1111111111111111111111111111111111111111111111111111111111111111'/></source>",
            "<source><file origin_sha256='1111111111111111111111111111111111111111111111111111111111111111'/></source>",
        ),
        (
            "file-size-format",
            "<source><file size='7'/></source>",
            "<source><file size='0007'/></source>",
        ),
    ];
    assert_named_game_changes(&mut catalog, &cases)?;
    Ok(())
}

#[test]
fn repeated_game_names_compare_fact_multisets_not_generated_ids_or_list_positions() -> TestResult {
    let mut catalog = Catalog::new()?;
    let before = "<datafile><game name='same'><archive number='duplicate' name='A'/></game><game name='same'><archive number='duplicate' name='B'/></game></datafile>";
    let after = "<datafile><game name='same'><archive number='duplicate' name='B'/></game><game name='same'><archive number='duplicate' name='A'/></game></datafile>";
    let diff = catalog.diff(before, after)?;
    assert_eq!(diff.records.len(), 1);
    assert_eq!(
        diff.records[0].correspondence,
        crate::domain::SnapshotRecordCorrespondence::ExactFacts
    );
    assert!(!diff.records[0].metadata_changed);
    assert!(diff.records[0].requirement_changes.is_empty());
    Ok(())
}

#[test]
fn export_history_does_not_expand_empty_root_relationships() -> TestResult {
    let mut catalog = Catalog::new()?;
    let before = catalog.import("<datafile><header><version>before</version></header><game name='same'><archive name='before'/></game></datafile>")?;
    let after = catalog.import("<datafile><header><version>after</version></header><game name='same'><archive name='after'/></game></datafile>")?;
    let queries = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&queries);
    {
        let mut connection = catalog.database.pool().get()?;
        connection.set_instrumentation(move |event: InstrumentationEvent<'_>| {
            if let InstrumentationEvent::StartQuery { query, .. } = event {
                captured
                    .lock()
                    .expect("query capture mutex")
                    .push(query.to_string());
            }
        });
    }
    let diff = catalog.diff_snapshots(&before, &after)?;
    assert!(diff.document_metadata_changed);
    let [record] = diff.records.as_slice() else {
        panic!("one native game history record expected");
    };
    assert!(record.metadata_changed);
    assert!(record.requirement_changes.is_empty());

    let explanation_queries = queries
        .lock()
        .expect("query capture mutex")
        .iter()
        .filter(|query| query.contains("scoped_assertions AS MATERIALIZED"))
        .count();
    assert_eq!(
        explanation_queries, 0,
        "proven-empty root relationships must not expand the full explanation queries"
    );
    Ok(())
}

#[test]
fn export_history_keeps_owned_decisions_supports_and_reviews() -> TestResult {
    use crate::domain::{
        RelationshipClaim, RelationshipEndpoint, RelationshipEvidence, RelationshipOrigin,
        RelationshipReview, RelationshipReviewDecision, RelationshipRule, RelationshipType,
    };
    let mut catalog = Catalog::new()?;
    let xml = "<datafile><game name='same'><archive number='1' clone='unresolved'/></game><game name='same'><archive number='2'/></game></datafile>";
    let before = catalog.import(xml)?;
    let after = catalog.import(&xml.replace("number='2'", "number='3'"))?;
    let owners = catalog.root_endpoints(&before)?;
    let [first, second] = owners.as_slice() else {
        panic!("two actual duplicate-name owners expected");
    };
    let base = RelationshipClaim {
        relation_type: RelationshipType::CatalogCorrection,
        subject: first.clone(),
        target: second.clone(),
        origin: RelationshipOrigin::UserConclusion,
        evidence: RelationshipEvidence::Rationale {
            reason: "actual duplicate owner witness".into(),
        },
    };
    let manual = app::record_relationship(&catalog.database, &base)?;
    let reverse = app::record_relationship(
        &catalog.database,
        &RelationshipClaim {
            subject: base.target.clone(),
            target: base.subject.clone(),
            ..base.clone()
        },
    )?;
    let supports = vec![reverse.clone(), manual.clone()];
    let inferred = app::record_relationship(
        &catalog.database,
        &RelationshipClaim {
            origin: RelationshipOrigin::DerivedCandidate {
                rule: RelationshipRule::new("history-witness", "v1", "Owned history candidate")?,
                supporting_assertions: supports.clone(),
            },
            ..base
        },
    )?;
    app::review_relationship(
        &catalog.database,
        &manual,
        &RelationshipReview {
            decision: RelationshipReviewDecision::Accepted,
            note: "accepted owner-specific correction".into(),
            superseded_by: None,
        },
    )?;
    let diff = catalog.diff_snapshots(&before, &after)?;
    let [record] = diff.records.as_slice() else {
        panic!("one duplicate-name history group expected")
    };
    assert_eq!(record.relationship_evidence.len(), 3);
    let accepted = record
        .relationship_evidence
        .iter()
        .find(|row| row.assertion_key == manual)
        .expect("manual correction");
    assert_eq!(accepted.review_history.len(), 1);
    assert_eq!(
        accepted.review_history[0].review.decision,
        RelationshipReviewDecision::Accepted
    );
    let candidate = record
        .relationship_evidence
        .iter()
        .find(|row| row.assertion_key == inferred)
        .expect("inferred candidate");
    assert!(
        matches!(&candidate.claim.origin, RelationshipOrigin::DerivedCandidate { supporting_assertions, .. } if *supporting_assertions == supports)
    );
    assert!(
        record
            .relationship_evidence
            .iter()
            .any(|row| row.assertion_key == reverse)
    );
    let all = app::explain_relationships(&catalog.database)?;
    assert!(all.iter().any(|row| matches!(
        row.claim.subject,
        RelationshipEndpoint::NoIntroArchive { .. }
    )));
    Ok(())
}

#[test]
fn export_history_keeps_reported_relationships_across_parser_formats() -> TestResult {
    let mut catalog = Catalog::new()?;
    let export =
        catalog.import("<datafile><game name='same'><archive number='1'/></game></datafile>")?;
    let xml = "<datafile><game name='same' cloneof='base'><description>Child</description></game><game name='base'><description>Base</description></game></datafile>";
    for mode in [
        crate::logiqx::LogiqxMode::ObservedCompatible,
        crate::logiqx::LogiqxMode::StrictDtd15,
    ] {
        let reported = catalog.import_with_format(xml, CatalogDocumentFormat::Logiqx(mode))?;
        for (before, after) in [(&export, &reported), (&reported, &export)] {
            let diff = catalog.diff_snapshots(before, after)?;
            let record = diff
                .records
                .iter()
                .find(|row| row.set_name == "same")
                .expect("shared root name");
            assert!(record.relationship_evidence.iter().any(|row| matches!(&row.claim.origin,
                crate::domain::RelationshipOrigin::SourceAssertion { snapshot, .. } if *snapshot == reported)));
        }
    }
    Ok(())
}

#[test]
fn export_history_keeps_incoming_actual_and_unresolved_root_decisions() -> TestResult {
    use crate::domain::{
        CatalogRecordKind, CatalogRecordRef, RelationshipClaim, RelationshipEndpoint,
        RelationshipEvidence, RelationshipOrigin, RelationshipType,
    };

    let mut catalog = Catalog::new()?;
    let before =
        catalog.import("<datafile><game name='same'><archive number='1'/></game></datafile>")?;
    let after =
        catalog.import("<datafile><game name='same'><archive number='2'/></game></datafile>")?;
    let outside = catalog.import("<datafile><game name='outside'/></datafile>")?;
    let owners = catalog.root_endpoints(&before)?;
    let [actual] = owners.as_slice() else {
        panic!("one actual root owner expected");
    };
    let unresolved = RelationshipEndpoint::CatalogRecord(CatalogRecordRef::new(
        before.clone(),
        CatalogRecordKind::Set,
        "same",
    ));
    let subject = RelationshipEndpoint::CatalogRecord(CatalogRecordRef::new(
        outside,
        CatalogRecordKind::Set,
        "outside",
    ));
    let mut incoming = Vec::new();
    for target in [actual.clone(), unresolved] {
        incoming.push(app::record_relationship(
            &catalog.database,
            &RelationshipClaim {
                relation_type: RelationshipType::CatalogCorrection,
                subject: subject.clone(),
                target,
                origin: RelationshipOrigin::UserConclusion,
                evidence: RelationshipEvidence::Rationale {
                    reason: "incoming root decision".into(),
                },
            },
        )?);
    }
    for (previous, current) in [(&before, &after), (&after, &before)] {
        let diff = catalog.diff_snapshots(previous, current)?;
        let [record] = diff.records.as_slice() else {
            panic!("one root history record expected");
        };
        assert_eq!(record.relationship_evidence.len(), incoming.len());
        for key in &incoming {
            assert!(
                record
                    .relationship_evidence
                    .iter()
                    .any(|row| &row.assertion_key == key)
            );
        }
    }
    Ok(())
}

#[test]
fn unknown_history_parser_format_is_not_an_empty_relationship_scope() -> TestResult {
    use diesel::{RunQueryDsl, sql_query, sql_types::Text};

    let mut catalog = Catalog::new()?;
    let known = catalog.import("<datafile/>")?;
    let unknown = SnapshotKey::from_persisted("unknown-history-edition".into());
    let mut connection = catalog.database.pool().get()?;
    sql_query("INSERT INTO parser_interpretations(interpretation_key,format) VALUES('unknown-history-interpretation','unsupported-history-format')").execute(&mut connection)?;
    sql_query("INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,acquisition_key,coverage_id) SELECT ?,catalog_key,document_key,'unknown-history-interpretation',acquisition_key,coverage_id FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text, _>(unknown.as_str()).bind::<Text, _>(known.as_str()).execute(&mut connection)?;
    drop(connection);
    assert!(
        matches!(catalog.diff_snapshots(&known, &unknown), Err(error) if error.to_string().contains("unsupported-history-format"))
    );
    Ok(())
}
