#![allow(clippy::expect_used, clippy::panic)]

use std::fmt::Write as _;

use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, CatalogSnapshotDiff, PublishingSourceKey, SnapshotKey},
    no_intro_db_xml::NoIntroDatabaseMode,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct Catalog {
    directory: tempfile::TempDir,
    database: Database,
    next_document: usize,
}

impl Catalog {
    fn new() -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
            .map_err(|_| "non-UTF-8 database path")?;
        Ok(Self {
            database: Database::open(&path)?,
            directory,
            next_document: 0,
        })
    }

    fn import(&mut self, xml: &str) -> TestResult<SnapshotKey> {
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
                format: CatalogDocumentFormat::NoIntroDatabase(
                    NoIntroDatabaseMode::ObservedCompatible,
                ),
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
            mame_coalesce::domain::SnapshotRecordCorrespondence::UniqueName,
            "{name}"
        );
    }
    Ok(())
}

fn owner_games(fields: &[&str], owner: &impl Fn(&str) -> String, value: Option<&str>) -> String {
    let mut games = String::new();
    for field in fields {
        let attributes = value
            .map(|value| format!("{field}='{value}'"))
            .unwrap_or_default();
        write!(
            games,
            "<game name='field-{field}'>{}</game>",
            owner(&attributes)
        )
        .expect("writing XML fixture to a String cannot fail");
    }
    games
}

fn assert_field_records(diff: &CatalogSnapshotDiff, fields: &[&str]) {
    let mut expected = fields
        .iter()
        .map(|field| format!("field-{field}"))
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
    for fields in [
        "",
        "<version/>",
        "<version>one</version><version>two</version>",
        "<version>one</version><version>one</version>",
    ] {
        let xml = format!(
            "<datafile><header>{fields}</header><game name='same'><archive clone='2'/></game></datafile>"
        );
        let snapshot = catalog.import(&xml)?;
        let history =
            app::catalog_snapshot_history(&catalog.database, &CatalogKey::new("export-consumers"))?;
        let entry = history
            .iter()
            .find(|entry| entry.snapshot == snapshot)
            .expect("history entry");
        assert_eq!(
            entry.declared_version.as_deref(),
            (fields == "<version/>").then_some(""),
            "{fields}"
        );
        let explanations = app::explain_relationships(&catalog.database)?;
        let explanation = explanations
            .iter()
            .find(|explanation| {
                matches!(
                    &explanation.claim.subject,
                    mame_coalesce::domain::RelationshipEndpoint::NoIntroArchive {
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
    for after in [
        "<datafile><header><version>two</version><author>A</author></header><game name='same'/></datafile>",
        "<datafile><header><author>A</author><version>one</version></header><game name='same'/></datafile>",
        "<header><version>one</version><author>A</author></header><datafile><game name='same'/></datafile>",
    ] {
        let diff = catalog.diff(before, after)?;
        assert!(diff.document_metadata_changed, "{after}");
        assert!(!diff.records[0].metadata_changed);
    }
    let diff = catalog.diff(
        "<datafile><game name='same'/></datafile>",
        "<datafile><header/><game name='same'/></datafile>",
    )?;
    assert!(diff.document_metadata_changed);
    Ok(())
}

#[test]
fn all_archive_attributes_are_visible_to_history() -> TestResult {
    assert_owner_fields(
        &[
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
        ],
        |attributes| format!("<archive {attributes}/>"),
    )
}

#[test]
fn all_dump_details_and_serials_are_visible_to_history() -> TestResult {
    assert_owner_fields(
        &[
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
        ],
        |attributes| format!("<source><details {attributes}/></source>"),
    )?;
    assert_owner_fields(
        &[
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
        ],
        |attributes| format!("<source><serials {attributes}/></source>"),
    )
}

#[test]
fn all_release_details_serials_and_nfo_aliases_are_visible_to_history() -> TestResult {
    assert_owner_fields(
        &[
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
        ],
        |attributes| format!("<release><details {attributes}/></release>"),
    )?;
    assert_owner_fields(
        &[
            "box_barcode",
            "box_serial",
            "media_serial1",
            "mediastamp",
            "pcb_serial",
            "romchip_serial1",
        ],
        |attributes| format!("<release><serials {attributes}/></release>"),
    )
}

#[test]
fn all_source_and_release_file_declarations_remain_scoped_metadata() -> TestResult {
    assert_owner_fields(
        &[
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
        ],
        |attributes| format!("<source><file {attributes}/></source>"),
    )?;
    assert_owner_fields(
        &[
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
        ],
        |attributes| format!("<release><file {attributes}/></release>"),
    )
}

fn assert_owner_fields(fields: &[&str], owner: impl Fn(&str) -> String) -> TestResult {
    let mut catalog = Catalog::new()?;
    let before = format!(
        "<datafile>{}</datafile>",
        owner_games(fields, &owner, Some("before"))
    );
    let after = format!(
        "<datafile>{}</datafile>",
        owner_games(fields, &owner, Some("after"))
    );
    let absent = format!("<datafile>{}</datafile>", owner_games(fields, &owner, None));
    let empty = format!(
        "<datafile>{}</datafile>",
        owner_games(fields, &owner, Some(""))
    );
    let before_snapshot = catalog.import(&before)?;
    let after_snapshot = catalog.import(&after)?;
    let absent_snapshot = catalog.import(&absent)?;
    let empty_snapshot = catalog.import(&empty)?;
    assert_eq!(
        catalog.next_document, 4,
        "one import per comparison document"
    );

    let value_diff = catalog.diff_snapshots(&before_snapshot, &after_snapshot)?;
    let empty_diff = catalog.diff_snapshots(&absent_snapshot, &empty_snapshot)?;
    for (diff, metadata_message) in [
        (&value_diff, "missing native field"),
        (&empty_diff, "empty/present collapsed"),
    ] {
        assert_field_records(diff, fields);
        assert!(!diff.document_metadata_changed);
        for record in &diff.records {
            assert_eq!(
                record.correspondence,
                mame_coalesce::domain::SnapshotRecordCorrespondence::UniqueName,
                "field game must have one-to-one history correspondence: {}",
                record.set_name
            );
            assert!(
                record.metadata_changed,
                "{metadata_message} for {}",
                record.set_name
            );
            assert!(
                record.requirement_changes.is_empty(),
                "unknown-scope declaration became requirement: {}",
                record.set_name
            );
        }
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
    for (before, after) in [
        ("<archive clone='P'/>", "<archive clone='1'/>"),
        ("<archive clone='1'/>", "<archive mergeof='1'/>"),
        (
            "<release><details nfocrc='11111111'/></release>",
            "<release><details nfo_crc32='11111111'/></release>",
        ),
        (
            "<source><file sha256='1111111111111111111111111111111111111111111111111111111111111111'/></source>",
            "<source><file origin_sha256='1111111111111111111111111111111111111111111111111111111111111111'/></source>",
        ),
        (
            "<source><file size='7'/></source>",
            "<source><file size='0007'/></source>",
        ),
    ] {
        let diff = catalog.diff(&document(before), &document(after))?;
        assert!(diff.records[0].metadata_changed, "{before} => {after}");
        assert!(diff.records[0].requirement_changes.is_empty());
    }
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
        mame_coalesce::domain::SnapshotRecordCorrespondence::ExactFacts
    );
    assert!(!diff.records[0].metadata_changed);
    assert!(diff.records[0].requirement_changes.is_empty());
    Ok(())
}
