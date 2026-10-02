#![allow(clippy::expect_used, clippy::panic)]

use camino::Utf8PathBuf;
use diesel::{QueryableByName, RunQueryDsl, sql_query, sql_types::Text};
use tempfile::TempDir;

use super::*;
use crate::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogScope, PublishingSourceKey},
};

struct Fixture {
    directory: TempDir,
    database: Database,
}

#[derive(Debug, QueryableByName)]
struct ImportDiagnostic {
    #[diesel(sql_type = Text)]
    code: String,
    #[diesel(sql_type = Text)]
    message: String,
}

fn fixture() -> Fixture {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .expect("UTF-8 database path");
    Fixture {
        directory,
        database: Database::open(&path).expect("open database"),
    }
}

fn import(fixture: &Fixture, path: &std::path::Path, format: CatalogDocumentFormat, key: &str) {
    let path = Utf8PathBuf::from_path_buf(path.to_owned()).expect("UTF-8 document path");
    let request = CatalogImportRequest {
        document_path: path,
        format,
        source_key: PublishingSourceKey::new(format!("source-{key}")),
        source_display_name: format!("Source {key}"),
        catalog_key: CatalogKey::new(key),
        catalog_display_name: format!("Catalog {key}"),
        scope: CatalogScope::Complete,
    };
    let report = app::import_catalog(&fixture.database, &request).expect("import catalog");
    if report.status != CatalogImportStatus::Succeeded {
        let mut connection = fixture
            .database
            .pool()
            .get()
            .expect("get database connection");
        let diagnostics = sql_query(
            "SELECT code, message FROM import_diagnostics WHERE run_key = ? ORDER BY rowid",
        )
        .bind::<Text, _>(report.run_key.to_string())
        .load::<ImportDiagnostic>(&mut connection)
        .expect("load import diagnostics")
        .into_iter()
        .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
        .collect::<Vec<_>>();
        panic!("catalog import failed: {diagnostics:?}");
    }
}

fn write_logiqx(path: &std::path::Path, header: &str, set: &str) {
    std::fs::write(
        path,
        format!(
            "<datafile><header><name>{header}</name></header><game name=\"{set}\"><rom name=\"game.bin\" size=\"1\"/></game></datafile>"
        ),
    )
    .expect("write Logiqx DAT");
}

fn load(fixture: &Fixture, key: &str) -> PublishedBuildCatalog {
    BuildCatalogRepository::new(fixture.database.pool())
        .load(&CatalogSelector::LatestCatalog(key.to_owned()))
        .expect("load published build catalog")
}

#[test]
fn no_intro_catalog_selector_uses_native_header_name() {
    let fixture = fixture();
    let path = fixture.directory.path().join("no-intro.dat");
    std::fs::write(
        &path,
        "<datafile><header><id>1</id><name>Header label</name>\
         <description>DAT description</description><version>native-v7</version></header>\
         <game name='root' cloneof='parent'><description>Root</description><rom name='root.bin' size='1'/></game></datafile>",
    )
    .expect("write No-Intro DAT");
    import(
        &fixture,
        &path,
        CatalogDocumentFormat::NoIntroDat(crate::NoIntroDatMode::V4Compatible),
        "no-intro-selector",
    );

    let selected = BuildCatalogRepository::new(fixture.database.pool())
        .load(&CatalogSelector::LatestCatalog("Header label".to_owned()))
        .expect("load selected No-Intro catalog");
    assert_eq!(selected.requirements().len(), 1);
    assert_eq!(
        selected.requirements()[0].parent_name.as_deref(),
        Some("parent")
    );
}

#[test]
fn logiqx_root_metadata_sparse_hashes_and_large_size_survive_loading() {
    let fixture = fixture();
    let path = fixture.directory.path().join("root.dat");
    std::fs::write(
        &path,
        r#"<datafile><header><name>Roots</name></header><game name="root" sourcefile="driver.cpp" isbios="yes" romof="bios" sampleof="samples" board="board-a" rebuildto="rebuilt"><description>Root description</description><year>1988</year><manufacturer>Maker</manufacturer><device_ref name="sound"/><rom name="large.bin" size="4294967296"/></game></datafile>"#,
    )
    .expect("write Logiqx DAT");
    import(
        &fixture,
        &path,
        CatalogDocumentFormat::Logiqx,
        "logiqx-root",
    );

    let catalog = load(&fixture, "logiqx-root");
    assert_eq!(catalog.requirements().len(), 1);
    let rom = &catalog.requirements()[0];
    assert_eq!(rom.role, AssetRole::Rom);
    assert_eq!(rom.expected.size, Some(4_294_967_296));
    assert_eq!(rom.expected.md5, None);
    assert_eq!(rom.expected.sha1, None);
    assert_eq!(rom.set_metadata.source_file.as_deref(), Some("driver.cpp"));
    assert_eq!(rom.set_metadata.is_bios.as_deref(), Some("yes"));
    assert_eq!(rom.set_metadata.rom_of.as_deref(), Some("bios"));
    assert_eq!(rom.set_metadata.sample_of.as_deref(), Some("samples"));
    assert_eq!(rom.set_metadata.device_refs, ["sound"]);
    assert_eq!(rom.set_metadata.board.as_deref(), Some("board-a"));
    assert_eq!(rom.set_metadata.rebuild_to.as_deref(), Some("rebuilt"));
}

#[test]
fn mame_root_metadata_and_rom_only_requirements_survive_loading() {
    let fixture = fixture();
    let path = fixture.directory.path().join("mame.xml");
    std::fs::write(
        &path,
        r#"<mame build="synthetic" debug="no" mameconfig="10"><machine name="machine" sourcefile="machine.cpp" isbios="yes" romof="bios" sampleof="samples"><description>Machine description</description><year>1991</year><manufacturer>MAME maker</manufacturer><device_ref tag=":sound" name="sound"/><rom name="game.bin" size="1" crc="12345678"/><disk name="disk" sha1="0123456789abcdef0123456789abcdef01234567"/><sample name="effect.wav"/></machine></mame>"#,
    )
    .expect("write MAME XML");
    import(
        &fixture,
        &path,
        CatalogDocumentFormat::MameListXml,
        "mame-root",
    );

    let catalog = load(&fixture, "mame-root");
    assert_eq!(catalog.requirements().len(), 1);
    let rom = &catalog.requirements()[0];
    assert_eq!(rom.role, AssetRole::Rom);
    assert_eq!(rom.set_metadata.source_file.as_deref(), Some("machine.cpp"));
    assert_eq!(rom.set_metadata.is_bios.as_deref(), Some("yes"));
    assert_eq!(rom.set_metadata.rom_of.as_deref(), Some("bios"));
    assert_eq!(rom.set_metadata.sample_of.as_deref(), Some("samples"));
    assert_eq!(rom.set_metadata.device_refs, ["sound"]);
    assert_eq!(
        rom.set_metadata.description.as_deref(),
        Some("Machine description")
    );
    assert_eq!(rom.set_metadata.year.as_deref(), Some("1991"));
    assert_eq!(rom.set_metadata.manufacturer.as_deref(), Some("MAME maker"));
}

#[test]
fn clrmamepro_root_metadata_excludes_sample_requirements() {
    let fixture = fixture();
    let path = fixture.directory.path().join("root.cmp");
    std::fs::write(
        &path,
        "clrmamepro ( name \"CMP roots\" )\ngame ( name \"root\" description \"CMP description\" year 1998 manufacturer \"CMP maker\" sampleof \"samples\" sample \"effect\" rom ( name \"game.bin\" size 1 crc 12345678 ) )\n",
    )
    .expect("write CMP DAT");
    import(
        &fixture,
        &path,
        CatalogDocumentFormat::ClrMamePro,
        "cmp-root",
    );

    let catalog = load(&fixture, "cmp-root");
    assert_eq!(catalog.requirements().len(), 1);
    let rom = &catalog.requirements()[0];
    assert_eq!(rom.role, AssetRole::Rom);
    assert_eq!(
        rom.set_metadata.description.as_deref(),
        Some("CMP description")
    );
    assert_eq!(rom.set_metadata.year.as_deref(), Some("1998"));
    assert_eq!(rom.set_metadata.manufacturer.as_deref(), Some("CMP maker"));
    assert_eq!(rom.set_metadata.sample_of.as_deref(), Some("samples"));
}

#[test]
fn no_intro_root_metadata_and_pc_parent_lookup_are_snapshot_scoped() {
    let fixture = fixture();
    let a_path = fixture.directory.path().join("pc-a.xml");
    let b_path = fixture.directory.path().join("pc-b.xml");
    std::fs::write(
        &a_path,
        r#"<datafile><game name="parent-old" id="77"><description>Old parent</description><rom name="parent.bin" size="1"/></game><game name="child" id="88" clone="77"><description>Child description</description><rom name="child.bin" size="1"/></game></datafile>"#,
    )
    .expect("write first PC snapshot");
    import(
        &fixture,
        &a_path,
        CatalogDocumentFormat::NoIntroPcXml,
        "pc-a",
    );
    std::fs::write(
        &b_path,
        r#"<datafile><game name="foreign-parent" id="77"><description>Foreign parent</description><rom name="foreign.bin" size="1"/></game></datafile>"#,
    )
    .expect("write other catalog");
    import(
        &fixture,
        &b_path,
        CatalogDocumentFormat::NoIntroPcXml,
        "pc-b",
    );
    std::fs::write(
        &a_path,
        r#"<datafile><game name="parent-new" id="77"><description>Current parent</description><rom name="parent.bin" size="1"/></game><game name="child" id="88" clone="77"><description>Child description</description><rom name="child.bin" size="1"/></game></datafile>"#,
    )
    .expect("write reimported PC snapshot");
    import(
        &fixture,
        &a_path,
        CatalogDocumentFormat::NoIntroPcXml,
        "pc-a",
    );

    let catalog = load(&fixture, "pc-a");
    assert_eq!(catalog.requirements().len(), 2);
    assert!(
        catalog
            .requirements()
            .iter()
            .all(|rom| rom.role == AssetRole::Rom)
    );
    let child = catalog
        .requirements()
        .iter()
        .find(|rom| rom.key.game_name() == "child")
        .expect("child requirement");
    assert_eq!(child.parent_name.as_deref(), Some("parent-new"));
    assert_eq!(
        child.set_metadata.description.as_deref(),
        Some("Child description")
    );
}

#[test]
fn software_lists_are_not_flattened_into_root_build_catalogs() {
    let fixture = fixture();
    let path = fixture.directory.path().join("software.xml");
    std::fs::write(
        &path,
        r#"<softwarelists><softwarelist name="list"><software name="item"><description>Item</description><year>2000</year><publisher>Maker</publisher><part name="cart" interface="cart"/></software></softwarelist></softwarelists>"#,
    )
    .expect("write software list");
    import(
        &fixture,
        &path,
        CatalogDocumentFormat::MameSoftwareListXml,
        "software",
    );
    assert!(
        BuildCatalogRepository::new(fixture.database.pool())
            .load(&CatalogSelector::LatestCatalog("software".to_owned()))
            .is_err()
    );
}

#[test]
fn renamed_header_alias_uses_current_publications_without_stale_aliases() {
    let fixture = fixture();
    let a_path = fixture.directory.path().join("header-a.dat");
    let b_path = fixture.directory.path().join("header-b.dat");
    write_logiqx(&a_path, "Old", "set-a");
    import(&fixture, &a_path, CatalogDocumentFormat::Logiqx, "header-a");
    write_logiqx(&a_path, "New", "set-a");
    import(&fixture, &a_path, CatalogDocumentFormat::Logiqx, "header-a");
    write_logiqx(&b_path, "Old", "set-b");
    import(&fixture, &b_path, CatalogDocumentFormat::Logiqx, "header-b");

    let old_alias = load(&fixture, "Old");
    let new_alias = load(&fixture, "New");
    let exact_key = load(&fixture, "header-a");
    assert_eq!(old_alias.catalog_key().as_str(), "header-b");
    assert_eq!(new_alias.catalog_key().as_str(), "header-a");
    assert_eq!(exact_key.catalog_key().as_str(), "header-a");
    assert_eq!(exact_key.requirements()[0].key.game_name(), "set-a");
}
