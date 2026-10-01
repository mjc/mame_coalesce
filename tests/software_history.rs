use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{
        CatalogKey, CatalogScope, PublishingSourceKey, QualifiedCatalogSet, SetName, SnapshotKey,
        SnapshotRecordCorrespondence, SnapshotRecordStatus,
    },
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn import(
    database: &Database,
    directory: &tempfile::TempDir,
    name: &str,
    xml: &str,
) -> TestResult<SnapshotKey> {
    import_scoped(database, directory, name, xml, CatalogScope::Complete)
}

fn import_scoped(
    database: &Database,
    directory: &tempfile::TempDir,
    name: &str,
    xml: &str,
    scope: CatalogScope,
) -> TestResult<SnapshotKey> {
    let path =
        Utf8PathBuf::from_path_buf(directory.path().join(name)).map_err(|_| "non-UTF-8 path")?;
    std::fs::write(&path, xml)?;
    let report = app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path: path,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("software-history"),
            source_display_name: "Software history".into(),
            catalog_key: CatalogKey::new("software-history"),
            catalog_display_name: "Software history".into(),
            scope,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    report.snapshot_key.ok_or_else(|| "snapshot missing".into())
}

fn document(metadata: &str) -> String {
    format!(
        r#"<softwarelist name="nes"><software name="game"><description>Game</description><year>2000</year><publisher>Publisher</publisher>{metadata}</software></softwarelist>"#
    )
}

fn open_database(directory: &tempfile::TempDir) -> TestResult<Database> {
    let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 path")?;
    Database::open(&path).map_err(Into::into)
}

const NATIVE: &str = r#"<softwarelists build="original"><softwarelist name="nes" description="NES"><notes>list notes</notes>
<software name="game" cloneof="parent" supported="partial"><description>Game</description><year>2000</year><publisher>Publisher</publisher><notes>item notes</notes>
<info name="region" value="US"/><info name="empty" value=""/><sharedfeat name="shared" value="shared value"/>
<part name="cart" interface="cart"><feature name="mapper" value="mapper value"/>
<dataarea name="rom" size="020" width="16" endianness="big"><rom name="file.bin" size="010" offset="0x10" crc="12345678" sha1="1111111111111111111111111111111111111111" status="baddump" loadflag="load16_byte"/>
<rom size="2" offset="2" value="ff" loadflag="continue"/></dataarea>
<diskarea name="disk"><disk name="disk.chd" sha1="2222222222222222222222222222222222222222" status="nodump" writeable="yes"/></diskarea>
<dipswitch name="Mode" tag="MODE" mask="0x01"><dipvalue name="Off" value="0" default="no"/><dipvalue name="On" value="1" default="yes"/></dipswitch>
</part></software></softwarelist></softwarelists>"#;

#[test]
fn every_native_software_family_changes_history() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database = open_database(&directory)?;
    let before = import(&database, &directory, "original.xml", NATIVE)?;
    for (index, (old, new)) in [
        ("cloneof=\"parent\"", "cloneof=\"other\""),
        ("supported=\"partial\"", "supported=\"no\""),
        (">Game<", ">Other<"),
        (">2000<", ">2001<"),
        (">Publisher<", ">Other publisher<"),
        (">item notes<", ">other notes<"),
        ("value=\"US\"", "value=\"JP\""),
        ("name=\"empty\" value=\"\"", "name=\"empty\""),
        ("shared value", "other shared value"),
        ("interface=\"cart\"", "interface=\"other\""),
        ("mapper value", "other mapper value"),
        ("size=\"020\"", "size=\"0x10\""),
        ("width=\"16\"", "width=\"32\""),
        ("endianness=\"big\"", "endianness=\"little\""),
        ("size=\"010\"", "size=\"0x8\""),
        ("offset=\"0x10\"", "offset=\"020\""),
        ("crc=\"12345678\"", "crc=\"12345679\""),
        (
            "sha1=\"1111111111111111111111111111111111111111\"",
            "sha1=\"3333333333333333333333333333333333333333\"",
        ),
        ("status=\"baddump\"", "status=\"good\""),
        ("loadflag=\"load16_byte\"", "loadflag=\"load16_word\""),
        ("value=\"ff\"", "value=\"aa\""),
        ("loadflag=\"continue\"", "loadflag=\"reload\""),
        ("name=\"disk.chd\"", "name=\"other.chd\""),
        (
            "sha1=\"2222222222222222222222222222222222222222\"",
            "sha1=\"4444444444444444444444444444444444444444\"",
        ),
        ("status=\"nodump\"", "status=\"good\""),
        ("writeable=\"yes\"", "writeable=\"no\""),
        ("tag=\"MODE\"", "tag=\"OTHER\""),
        ("mask=\"0x01\"", "mask=\"1\""),
        ("name=\"Off\"", "name=\"Zero\""),
        ("value=\"0\"", "value=\"00\""),
        ("default=\"no\"", "default=\"yes\""),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(NATIVE.contains(old));
        let after = import(
            &database,
            &directory,
            &format!("change-{index}.xml"),
            &NATIVE.replacen(old, new, 1),
        )?;
        let diff = app::diff_catalog_snapshots(&database, &before, &after)?;
        assert_eq!(diff.records.len(), 1);
        assert_eq!(
            diff.records[0].status,
            SnapshotRecordStatus::Changed,
            "{old} -> {new}"
        );
        assert!(diff.records[0].metadata_changed, "{old} -> {new}");
    }
    Ok(())
}

#[test]
fn wrapper_and_list_metadata_are_document_changes_not_title_edits() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database = open_database(&directory)?;
    let before = import(&database, &directory, "original.xml", NATIVE)?;
    for (index, (old, new)) in [
        ("build=\"original\"", "build=\"new\""),
        ("description=\"NES\"", "description=\"Nintendo\""),
        (">list notes<", ">other list notes<"),
    ]
    .into_iter()
    .enumerate()
    {
        let after = import(
            &database,
            &directory,
            &format!("document-{index}.xml"),
            &NATIVE.replacen(old, new, 1),
        )?;
        let diff = app::diff_catalog_snapshots(&database, &before, &after)?;
        assert!(diff.document_metadata_changed);
        assert_eq!(diff.records[0].status, SnapshotRecordStatus::Unchanged);
    }
    let single = NATIVE
        .strip_prefix("<softwarelists build=\"original\">")
        .and_then(|xml| xml.strip_suffix("</softwarelists>"))
        .ok_or("missing wrapper")?;
    let after = import(&database, &directory, "single.xml", single)?;
    let diff = app::diff_catalog_snapshots(&database, &before, &after)?;
    assert!(diff.document_metadata_changed);
    assert_eq!(diff.records[0].status, SnapshotRecordStatus::Unchanged);
    Ok(())
}

#[test]
fn native_child_order_matters_but_vendor_gaps_and_physical_positions_do_not() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database = open_database(&directory)?;
    let before = import(&database, &directory, "original.xml", NATIVE)?;
    let vendor = NATIVE
        .replace("<notes>", "\n<vendor/>\n<notes>")
        .replace("<info", "\n<vendor/>\n<info")
        .replace("<dataarea", "\n<vendor/>\n<dataarea")
        .replace("<rom ", "\n<vendor/>\n<rom ")
        .replace("<dipvalue ", "\n<vendor/>\n<dipvalue ");
    let after = import(&database, &directory, "vendor.xml", &vendor)?;
    let diff = app::diff_catalog_snapshots(&database, &before, &after)?;
    assert!(!diff.document_metadata_changed);
    assert_eq!(diff.records[0].status, SnapshotRecordStatus::Unchanged);
    let info = "<info name=\"region\" value=\"US\"/>";
    let shared = "<sharedfeat name=\"shared\" value=\"shared value\"/>";
    let swapped = NATIVE.replace(
        &format!("{info}<info name=\"empty\" value=\"\"/>{shared}"),
        &format!("{shared}<info name=\"empty\" value=\"\"/>{info}"),
    );
    let after = import(&database, &directory, "order.xml", &swapped)?;
    let diff = app::diff_catalog_snapshots(&database, &before, &after)?;
    assert_eq!(diff.records[0].status, SnapshotRecordStatus::Changed);
    Ok(())
}

#[test]
fn explicit_defaults_and_usable_hash_changes_are_distinct_history_facts() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database = open_database(&directory)?;
    let xml = document(
        r#"<part name="cart" interface="cart"><dataarea name="rom" size="8"><rom name="file.bin" size="8" sha1="1111111111111111111111111111111111111111"/></dataarea><diskarea name="media"><disk name="disk"/></diskarea><dipswitch name="Mode" tag="M" mask="1"><dipvalue name="Off" value="0"/></dipswitch></part>"#,
    );
    let before = import(&database, &directory, "implicit.xml", &xml)?;
    for (index, (old, new)) in [
        ("name=\"game\"", "name=\"game\" supported=\"yes\""),
        (
            "name=\"rom\" size=\"8\"",
            "name=\"rom\" size=\"8\" width=\"8\"",
        ),
        (
            "name=\"rom\" size=\"8\"",
            "name=\"rom\" size=\"8\" endianness=\"little\"",
        ),
        ("name=\"file.bin\"", "name=\"file.bin\" status=\"good\""),
        ("name=\"disk\"", "name=\"disk\" status=\"good\""),
        ("name=\"disk\"", "name=\"disk\" writeable=\"no\""),
        ("name=\"Off\"", "name=\"Off\" default=\"no\""),
    ]
    .into_iter()
    .enumerate()
    {
        let after = import(
            &database,
            &directory,
            &format!("explicit-{index}.xml"),
            &xml.replacen(old, new, 1),
        )?;
        let diff = app::diff_catalog_snapshots(&database, &before, &after)?;
        assert_eq!(
            diff.records[0].status,
            SnapshotRecordStatus::Changed,
            "{old}"
        );
        assert!(diff.records[0].metadata_changed);
        assert!(diff.records[0].requirement_changes.is_empty());
    }
    let after = import(
        &database,
        &directory,
        "hash.xml",
        &xml.replace(
            "1111111111111111111111111111111111111111",
            "2222222222222222222222222222222222222222",
        ),
    )?;
    let diff = app::diff_catalog_snapshots(&database, &before, &after)?;
    assert_eq!(diff.records[0].requirement_changes.len(), 1);
    assert!(diff.records[0].requirement_changes[0].hash_changed);
    assert!(!diff.records[0].requirement_changes[0].size_changed);
    Ok(())
}

#[test]
fn same_named_titles_in_different_lists_do_not_share_history_identity() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database = open_database(&directory)?;
    let xml = format!(
        "<softwarelists>{}{}</softwarelists>",
        document(""),
        document("").replace("name=\"nes\"", "name=\"snes\"")
    );
    let before = import(&database, &directory, "lists-before.xml", &xml)?;
    let after = import(
        &database,
        &directory,
        "lists-after.xml",
        &xml.replacen(">Game<", ">Different<", 1),
    )?;
    let diff = app::diff_catalog_snapshots(&database, &before, &after)?;
    assert_eq!(diff.records.len(), 2);
    assert_eq!(
        diff.records
            .iter()
            .filter(|row| row.status == SnapshotRecordStatus::Changed)
            .count(),
        1
    );
    assert_eq!(
        diff.records
            .iter()
            .filter(|row| row.status == SnapshotRecordStatus::Unchanged)
            .count(),
        1
    );
    assert!(
        diff.records
            .iter()
            .any(|row| row.set_name == r#"["nes","game"]"#)
    );
    assert!(
        diff.records
            .iter()
            .any(|row| row.set_name == r#"["snes","game"]"#)
    );
    Ok(())
}

#[test]
fn software_history_uses_qualified_coverage_for_missing_titles() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database = open_database(&directory)?;
    let included = QualifiedCatalogSet::SoftwareItem {
        list_name: "nes".into(),
        name: SetName::new("game"),
    };
    let filtered = CatalogScope::Filtered(std::iter::once(included).collect());
    let before = import_scoped(
        &database,
        &directory,
        "filtered.xml",
        &document(""),
        filtered,
    )?;
    let after = import(
        &database,
        &directory,
        "complete.xml",
        &document("").replace("name=\"game\"", "name=\"other\""),
    )?;
    let diff = app::diff_catalog_snapshots(&database, &before, &after)?;
    let removed = diff
        .records
        .iter()
        .find(|row| row.set_name == r#"["nes","game"]"#)
        .ok_or("removed game absent")?;
    let excluded = diff
        .records
        .iter()
        .find(|row| row.set_name == r#"["nes","other"]"#)
        .ok_or("excluded game absent")?;
    assert_eq!(removed.status, SnapshotRecordStatus::RemovedWithinScope);
    assert_eq!(excluded.status, SnapshotRecordStatus::OutOfScope);
    Ok(())
}

#[test]
fn repeated_qualified_title_names_compare_whole_owners_not_source_positions() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database = open_database(&directory)?;
    let first = "<software name=\"same\"><description>First</description><year>2000</year><publisher>P</publisher></software>";
    let second = first.replace(">First<", ">Second<");
    let before_xml = format!("<softwarelist name=\"nes\">{first}{second}</softwarelist>");
    let after_xml = format!("<softwarelist name=\"nes\">{second}{first}</softwarelist>");
    let before = import(&database, &directory, "repeated-before.xml", &before_xml)?;
    let after = import(&database, &directory, "repeated-after.xml", &after_xml)?;
    let diff = app::diff_catalog_snapshots(&database, &before, &after)?;
    assert_eq!(diff.records.len(), 1);
    assert_eq!(diff.records[0].status, SnapshotRecordStatus::Unchanged);
    assert_eq!(
        diff.records[0].correspondence,
        SnapshotRecordCorrespondence::ExactFacts
    );
    let changed_xml = before_xml.replace(">First<", ">Changed<");
    let changed = import(&database, &directory, "repeated-changed.xml", &changed_xml)?;
    let diff = app::diff_catalog_snapshots(&database, &before, &changed)?;
    assert_eq!(diff.records[0].status, SnapshotRecordStatus::Changed);
    assert_eq!(
        diff.records[0].correspondence,
        SnapshotRecordCorrespondence::Ambiguous
    );
    assert!(diff.records[0].requirement_changes.is_empty());
    Ok(())
}

#[test]
fn repeated_list_names_preserve_their_actual_title_ownership() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database = open_database(&directory)?;
    let list = |description: &str, publisher: &str| {
        format!(
            r#"<softwarelist name="nes" description="{description}"><software name="game"><description>Game</description><year>2000</year><publisher>{publisher}</publisher></software></softwarelist>"#
        )
    };
    let before_xml = format!(
        "<softwarelists>{}{}</softwarelists>",
        list("US", "A"),
        list("JP", "B")
    );
    let after_xml = format!(
        "<softwarelists>{}{}</softwarelists>",
        list("US", "B"),
        list("JP", "A")
    );
    let before = import(&database, &directory, "owners-before.xml", &before_xml)?;
    let after = import(&database, &directory, "owners-after.xml", &after_xml)?;
    let diff = app::diff_catalog_snapshots(&database, &before, &after)?;
    assert!(diff.document_metadata_changed);
    assert_eq!(diff.records.len(), 1);
    assert_eq!(diff.records[0].status, SnapshotRecordStatus::Changed);
    assert_eq!(
        diff.records[0].correspondence,
        SnapshotRecordCorrespondence::Ambiguous
    );
    assert!(diff.records[0].metadata_changed);
    let permutation = format!(
        "<softwarelists>{}{}</softwarelists>",
        list("JP", "B"),
        list("US", "A")
    );
    let reordered = import(&database, &directory, "owners-reordered.xml", &permutation)?;
    let diff = app::diff_catalog_snapshots(&database, &before, &reordered)?;
    assert_eq!(diff.records[0].status, SnapshotRecordStatus::Unchanged);
    assert_eq!(
        diff.records[0].correspondence,
        SnapshotRecordCorrespondence::ExactFacts
    );
    Ok(())
}

#[test]
fn unnamed_rom_source_entries_remain_metadata_not_invented_requirements() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database = open_database(&directory)?;
    let before_xml = document(
        r#"<part name="cart" interface="cart"><dataarea name="rom" size="8"><rom size="8" sha1="1111111111111111111111111111111111111111"/></dataarea></part>"#,
    );
    let after_xml = before_xml.replace(
        "1111111111111111111111111111111111111111",
        "2222222222222222222222222222222222222222",
    );
    let before = import(&database, &directory, "unnamed-before.xml", &before_xml)?;
    let after = import(&database, &directory, "unnamed-after.xml", &after_xml)?;
    let diff = app::diff_catalog_snapshots(&database, &before, &after)?;
    assert_eq!(diff.records[0].status, SnapshotRecordStatus::Changed);
    assert!(diff.records[0].metadata_changed);
    assert!(diff.records[0].requirement_changes.is_empty());
    Ok(())
}

#[test]
fn software_native_metadata_is_compared_and_whitespace_is_not_a_change() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 path")?;
    let database = Database::open(&path)?;
    let before = import(
        &database,
        &directory,
        "before.xml",
        &document(r#"<info name="region" value="US"/>"#),
    )?;
    let after = import(
        &database,
        &directory,
        "after.xml",
        &document(r#"<info name="region" value="JP"/>"#),
    )?;
    let diff = app::diff_catalog_snapshots(&database, &before, &after)?;
    assert_eq!(diff.records.len(), 1);
    assert_eq!(diff.records[0].status, SnapshotRecordStatus::Changed);
    assert!(diff.records[0].metadata_changed);
    let reindented = import(
        &database,
        &directory,
        "reindented.xml",
        &document("\n  <info name=\"region\" value=\"JP\"/>\n"),
    )?;
    let diff = app::diff_catalog_snapshots(&database, &after, &reindented)?;
    assert_eq!(diff.records[0].status, SnapshotRecordStatus::Unchanged);
    assert!(!diff.document_metadata_changed);
    Ok(())
}

#[test]
fn clone_history_keeps_relationship_evidence_in_its_actual_software_list() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database = open_database(&directory)?;
    let before_xml = r#"<softwarelists>
      <softwarelist name="nes"><software name="game" cloneof="nes-parent">
        <description>Game</description><year>2000</year><publisher>Publisher</publisher>
      </software></softwarelist>
      <softwarelist name="snes"><software name="game" cloneof="snes-parent">
        <description>Game</description><year>2000</year><publisher>Publisher</publisher>
      </software></softwarelist>
    </softwarelists>"#;
    let before = import(&database, &directory, "clones-before.xml", before_xml)?;
    let after = import(
        &database,
        &directory,
        "clones-after.xml",
        &before_xml.replace("cloneof=\"nes-parent\"", "cloneof=\"other-parent\""),
    )?;
    let diff = app::diff_catalog_snapshots(&database, &before, &after)?;
    assert_eq!(diff.records.len(), 2);
    for record in &diff.records {
        let name: [String; 2] = serde_json::from_str(&record.set_name)?;
        assert_eq!(name[1], "game");
        assert_eq!(record.relationship_evidence.len(), 2, "{name:?}");
        for explanation in &record.relationship_evidence {
            assert_eq!(explanation.source_field.as_deref(), Some("cloneof"));
            let mame_coalesce::domain::RelationshipEndpoint::CatalogRecord(subject) =
                &explanation.claim.subject
            else {
                return Err("clone subject must be its software title".into());
            };
            assert_eq!(
                subject.kind,
                mame_coalesce::domain::CatalogRecordKind::SoftwareItem
            );
            assert_eq!(subject.key.as_str(), record.set_name);
            assert!(subject.owner_set_id.is_some());
        }
        assert_eq!(
            record.status,
            if name[0] == "nes" {
                SnapshotRecordStatus::Changed
            } else {
                SnapshotRecordStatus::Unchanged
            }
        );
    }
    Ok(())
}
