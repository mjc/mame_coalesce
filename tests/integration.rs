use mame_coalesce::{
    app::{self, BuildWorkflowRequest, DatImportRequest, RunWorkflowRequest, SourceScanRequest},
    db::{self, create_db_pool},
    domain::BuildMode,
    logiqx::DataFile,
    operations,
    schema::rom_files::dsl::{rom_files, sha1},
};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read},
};

use diesel::prelude::*;

// =====================================================================
// DAT Parsing → DB Insert
// =====================================================================

const SIMPLE_DAT: &str = r#"<?xml version="1.0"?>
<datafile>
  <header>
    <name>Test Set</name>
    <description>Integration Test Data</description>
    <version>1.0</version>
    <author>Test</author>
  </header>
  <game name="pong" sourcefile="pong.c">
    <description>Pong</description>
    <year>1972</year>
    <manufacturer>Atari</manufacturer>
    <rom name="pong.rom" size="4096" sha1="a9993e364706816aba3e25717850c26c9cd0d89d" md5="900150983cd24fb0d6963f7d28e17f72" crc="12345678"/>
  </game>
  <game name="pong2" sourcefile="pong.c">
    <description>Pong 2</description>
    <year>1973</year>
    <manufacturer>Atari</manufacturer>
    <rom name="pong2.rom" size="8192" sha1="84983e441c3bd26ebaae4aa1f575527d004816f2" md5="f96b697d7cb7938d525a2f31aaf161d0" crc="aabbccdd"/>
  </game>
</datafile>"#;

const CLONE_DAT: &str = r#"<?xml version="1.0"?>
<datafile>
  <header>
    <name>Clone Test</name>
    <description>Test with parent/clone relationship</description>
    <version>1.0</version>
    <author>Test</author>
  </header>
  <game name="parent" sourcefile="parent.c">
    <description>Parent Game</description>
    <year>1980</year>
    <manufacturer>Acme</manufacturer>
    <rom name="parent.rom" size="8192" sha1="a9993e364706816aba3e25717850c26c9cd0d89d" md5="900150983cd24fb0d6963f7d28e17f72" crc="aabbccdd"/>
  </game>
  <game name="clone1" cloneof="parent" sourcefile="clone.c">
    <description>Clone Game 1</description>
    <year>1981</year>
    <manufacturer>Acme</manufacturer>
    <rom name="clone1.rom" size="4096" sha1="84983e441c3bd26ebaae4aa1f575527d004816f2" md5="f96b697d7cb7938d525a2f31aaf161d0" crc="bbccddee"/>
  </game>
  <game name="clone2" cloneof="parent" sourcefile="clone.c">
    <description>Clone Game 2</description>
    <year>1982</year>
    <manufacturer>Acme</manufacturer>
    <rom name="clone2.rom" size="4096" sha1="da39a3ee5e6b4b0d3255bfef95601890afd80709" md5="d41d8cd98f00b204e9800998ecf8427e" crc="ccddeeff"/>
  </game>
</datafile>"#;

fn in_memory_pool() -> mame_coalesce::Result<mame_coalesce::db::Pool> {
    create_db_pool(":memory:")
}

fn utf8_path(path: &std::path::Path) -> Result<&camino::Utf8Path, io::Error> {
    camino::Utf8Path::from_path(path).ok_or_else(|| io::Error::other("path is not UTF-8"))
}

fn write_clone_dat(dir: &std::path::Path) -> Result<camino::Utf8PathBuf, io::Error> {
    let dat_path = dir.join("clone.dat");
    fs::write(&dat_path, CLONE_DAT)?;
    Ok(utf8_path(&dat_path)?.to_path_buf())
}

fn write_present_clone_roms(dir: &std::path::Path) -> Result<camino::Utf8PathBuf, io::Error> {
    fs::write(dir.join("parent.rom"), b"abc")?;
    fs::write(dir.join("clone2.rom"), b"")?;
    Ok(utf8_path(dir)?.to_path_buf())
}

fn path_with_parent_component(path: &camino::Utf8Path) -> Result<camino::Utf8PathBuf, io::Error> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("path has no parent"))?;
    let grandparent = parent
        .parent()
        .ok_or_else(|| io::Error::other("path parent has no parent"))?;
    let parent_name = parent
        .file_name()
        .ok_or_else(|| io::Error::other("path parent has no file name"))?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("path has no file name"))?;

    Ok(grandparent
        .join(parent_name)
        .join("..")
        .join(parent_name)
        .join(name))
}

fn write_shared_dat(
    dir: &std::path::Path,
    file_name: &str,
    set_name: &str,
    rom_sha1: &str,
) -> Result<camino::Utf8PathBuf, io::Error> {
    let dat_path = dir.join(file_name);
    let dat = format!(
        r#"<?xml version="1.0"?>
<datafile>
  <header>
    <name>{set_name}</name>
    <description>Overlapping DAT Test</description>
    <version>1.0</version>
    <author>Test</author>
  </header>
  <game name="shared" sourcefile="shared.c">
    <description>Shared Game</description>
    <year>1980</year>
    <manufacturer>Acme</manufacturer>
    <rom name="shared.rom" size="4096" sha1="{rom_sha1}" md5="900150983cd24fb0d6963f7d28e17f72" crc="12345678"/>
  </game>
</datafile>"#
    );
    fs::write(&dat_path, dat)?;
    Ok(utf8_path(&dat_path)?.to_path_buf())
}

fn write_single_game_dat(
    path: &std::path::Path,
    set_name: &str,
    game_name: &str,
    rom_name: &str,
    rom_sha1: &str,
) -> Result<camino::Utf8PathBuf, io::Error> {
    let dat = format!(
        r#"<?xml version="1.0"?>
<datafile>
  <header>
    <name>{set_name}</name>
    <description>Reimport Test</description>
    <version>1.0</version>
    <author>Test</author>
  </header>
  <game name="{game_name}" sourcefile="reimport.c">
    <description>{game_name}</description>
    <year>1980</year>
    <manufacturer>Acme</manufacturer>
    <rom name="{rom_name}" size="4096" sha1="{rom_sha1}" md5="900150983cd24fb0d6963f7d28e17f72" crc="12345678"/>
  </game>
</datafile>"#
    );
    fs::write(path, dat)?;
    Ok(utf8_path(path)?.to_path_buf())
}

fn write_single_rom_source(
    dir: &std::path::Path,
    contents: &[u8],
) -> Result<camino::Utf8PathBuf, io::Error> {
    fs::write(dir.join("shared.rom"), contents)?;
    Ok(utf8_path(dir)?.to_path_buf())
}

fn zip_entries(
    path: &camino::Utf8Path,
) -> Result<BTreeMap<String, Vec<u8>>, Box<dyn std::error::Error>> {
    let file = fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut entries = BTreeMap::new();

    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;
        entries.insert(file.name().to_owned(), contents);
    }

    Ok(entries)
}

// =====================================================================
// DAT → DB insert
// =====================================================================

#[test]
fn parse_and_insert_dat_game_count() -> Result<(), Box<dyn std::error::Error>> {
    let pool = in_memory_pool()?;
    let df = DataFile::from_reader(SIMPLE_DAT.as_bytes())?;
    assert_eq!(df.games().len(), 2);
    db::traverse_and_insert_data_file(&pool, &df)?;
    Ok(())
}

#[test]
fn parse_and_insert_dat_rom_count() -> Result<(), Box<dyn std::error::Error>> {
    let pool = in_memory_pool()?;
    let df = DataFile::from_reader(SIMPLE_DAT.as_bytes())?;
    let total_roms: usize = df.games().iter().map(|g| g.roms().len()).sum();
    assert_eq!(total_roms, 2);
    db::traverse_and_insert_data_file(&pool, &df)?;
    Ok(())
}

#[test]
fn parse_and_insert_clone_dat() -> Result<(), Box<dyn std::error::Error>> {
    let pool = in_memory_pool()?;
    let df = DataFile::from_reader(CLONE_DAT.as_bytes())?;
    assert_eq!(df.games().len(), 3);
    let parent = df
        .games()
        .iter()
        .find(|g| g.name() == "parent")
        .ok_or_else(|| io::Error::other("missing parent game"))?;
    let clone1 = df
        .games()
        .iter()
        .find(|g| g.name() == "clone1")
        .ok_or_else(|| io::Error::other("missing clone game"))?;
    assert!(parent.cloneof().is_none());
    assert_eq!(
        clone1.cloneof().map(std::string::String::as_str),
        Some("parent")
    );
    db::traverse_and_insert_data_file(&pool, &df)?;
    Ok(())
}

#[test]
fn reimport_dat_replaces_previous_games_and_roms() -> Result<(), Box<dyn std::error::Error>> {
    use mame_coalesce::schema::{
        data_files::dsl::data_files,
        games::dsl::{games, name as game_name},
        roms::dsl::{name as rom_name, roms},
    };

    let pool = in_memory_pool()?;
    let dat_dir = tempfile::tempdir()?;
    let dat_file = dat_dir.path().join("mutable.dat");
    let dat_path = write_single_game_dat(
        &dat_file,
        "Mutable Set",
        "old-game",
        "old.rom",
        "a9993e364706816aba3e25717850c26c9cd0d89d",
    )?;

    operations::parse_and_insert_datfile(&dat_path, &pool)?;
    write_single_game_dat(
        &dat_file,
        "Mutable Set",
        "new-game",
        "new.rom",
        "da39a3ee5e6b4b0d3255bfef95601890afd80709",
    )?;
    operations::parse_and_insert_datfile(&dat_path, &pool)?;

    let mut conn = pool.get()?;
    assert_eq!(data_files.count().get_result::<i64>(&mut conn)?, 1);
    assert_eq!(games.count().get_result::<i64>(&mut conn)?, 1);
    assert_eq!(roms.count().get_result::<i64>(&mut conn)?, 1);

    let game_names = games.select(game_name).load::<String>(&mut conn)?;
    let rom_names = roms.select(rom_name).load::<String>(&mut conn)?;
    assert_eq!(game_names, vec!["new-game"]);
    assert_eq!(rom_names, vec!["new.rom"]);
    Ok(())
}

// =====================================================================
// Scan → DB
// =====================================================================

#[test]
fn scan_bare_files_inserts_rom_files() -> Result<(), Box<dyn std::error::Error>> {
    let pool = in_memory_pool()?;
    let dir = tempfile::tempdir()?;

    // Write a known file
    let content = b"rom content here";
    let rom_path = dir.path().join("game.rom");
    std::fs::write(&rom_path, content)?;

    let expected_sha1 = mame_coalesce::hashes::sha1_bytes(content);

    let utf8_dir = camino::Utf8Path::from_path(dir.path())
        .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;
    operations::source(utf8_dir, 0, &pool)?;

    // Verify something was inserted in the `rom_files` table.
    let mut conn = pool.get()?;
    let count: i64 = rom_files.count().get_result(&mut conn)?;
    assert_eq!(count, 1);

    // Verify the sha1 matches
    let sha1s: Vec<Vec<u8>> = rom_files.select(sha1).load(&mut conn)?;
    assert_eq!(sha1s[0], expected_sha1);
    Ok(())
}

#[test]
fn scan_zip_inserts_entries() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    let pool = in_memory_pool()?;
    let dir = tempfile::tempdir()?;

    // Build a zip with two entries
    let content_a = b"entry-a-content";
    let content_b = b"entry-b-content";
    let zip_path = dir.path().join("test.zip");
    {
        let f = std::fs::File::create(&zip_path)?;
        let mut zip = zip::ZipWriter::new(f);
        let opts = SimpleFileOptions::default();
        zip.start_file("a.rom", opts)?;
        zip.write_all(content_a)?;
        zip.start_file("b.rom", opts)?;
        zip.write_all(content_b)?;
        zip.finish()?;
    }

    let utf8_dir = camino::Utf8Path::from_path(dir.path())
        .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;
    operations::source(utf8_dir, 1, &pool)?;

    let mut conn = pool.get()?;
    let count: i64 = rom_files.count().get_result(&mut conn)?;
    assert_eq!(count, 2);
    Ok(())
}

#[test]
fn run_workflow_writes_parent_bundle_zip() -> Result<(), Box<dyn std::error::Error>> {
    let pool = in_memory_pool()?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = write_clone_dat(work_dir.path())?;
    let source_path = write_present_clone_roms(source_dir.path())?;
    let output_path = utf8_path(output_dir.path())?.to_path_buf();

    let report = app::run(
        &pool,
        &RunWorkflowRequest {
            dat_path,
            source_path,
            destination_path: output_path.clone(),
            mode: BuildMode::ParentBundles,
            jobs: 1,
            dry_run: false,
            strict: false,
        },
    )?;

    assert_eq!(report.exit_code, 0);
    assert_eq!(report.build_report.matched_roms, 2);
    assert_eq!(report.build_report.missing_roms.len(), 1);
    assert_eq!(report.build_report.missing_roms[0].rom_name, "clone1.rom");
    assert_eq!(report.written_paths, vec![output_path.join("parent.zip")]);

    let entries = zip_entries(&output_path.join("parent.zip"))?;
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries.get("parent.rom").map(Vec::as_slice),
        Some(b"abc" as &[u8])
    );
    assert_eq!(
        entries.get("clone2.rom").map(Vec::as_slice),
        Some(b"" as &[u8])
    );
    Ok(())
}

#[test]
fn dry_run_workflow_reports_plan_without_writing_files() -> Result<(), Box<dyn std::error::Error>> {
    let pool = in_memory_pool()?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = write_clone_dat(work_dir.path())?;
    let source_path = write_present_clone_roms(source_dir.path())?;
    let output_path = utf8_path(output_dir.path())?.join("dry-run-output");

    let report = app::run(
        &pool,
        &RunWorkflowRequest {
            dat_path,
            source_path,
            destination_path: output_path.clone(),
            mode: BuildMode::ParentBundles,
            jobs: 1,
            dry_run: true,
            strict: false,
        },
    )?;

    assert_eq!(report.exit_code, 0);
    assert!(report.dry_run);
    assert_eq!(report.build_report.matched_roms, 2);
    assert_eq!(report.build_report.missing_roms.len(), 1);
    assert!(report.written_paths.is_empty());
    assert!(!output_path.exists());
    Ok(())
}

#[test]
fn build_workflow_accepts_imported_dat_name() -> Result<(), Box<dyn std::error::Error>> {
    let pool = in_memory_pool()?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = write_clone_dat(work_dir.path())?;
    let source_path = write_present_clone_roms(source_dir.path())?;
    let output_path = utf8_path(output_dir.path())?.to_path_buf();

    app::import_dat(&pool, &DatImportRequest { dat_path })?;
    app::scan_source(
        &pool,
        &SourceScanRequest {
            source_path: source_path.clone(),
            jobs: 1,
        },
    )?;

    let report = app::build(
        &pool,
        &BuildWorkflowRequest {
            dat_path: camino::Utf8PathBuf::from("Clone Test"),
            source_path,
            destination_path: output_path.clone(),
            mode: BuildMode::ParentBundles,
            dry_run: false,
            strict: false,
        },
    )?;

    assert_eq!(report.exit_code, 0);
    assert_eq!(report.build_report.matched_roms, 2);
    assert_eq!(report.build_report.missing_roms.len(), 1);
    assert_eq!(report.build_report.missing_roms[0].rom_name, "clone1.rom");
    assert_eq!(report.written_paths, vec![output_path.join("parent.zip")]);

    let entries = zip_entries(&output_path.join("parent.zip"))?;
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries.get("parent.rom").map(Vec::as_slice),
        Some(b"abc" as &[u8])
    );
    assert_eq!(
        entries.get("clone2.rom").map(Vec::as_slice),
        Some(b"" as &[u8])
    );
    Ok(())
}

#[test]
fn build_matches_sources_scanned_from_noncanonical_path() -> Result<(), Box<dyn std::error::Error>>
{
    let pool = in_memory_pool()?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = write_clone_dat(work_dir.path())?;
    let source_path = write_present_clone_roms(source_dir.path())?;
    let scanned_source_path = path_with_parent_component(&source_path)?;
    let output_path = utf8_path(output_dir.path())?.to_path_buf();

    app::import_dat(
        &pool,
        &DatImportRequest {
            dat_path: dat_path.clone(),
        },
    )?;
    let scan_report = app::scan_source(
        &pool,
        &SourceScanRequest {
            source_path: scanned_source_path,
            jobs: 1,
        },
    )?;

    assert_eq!(scan_report.source_path, source_path.canonicalize_utf8()?);

    let report = app::build(
        &pool,
        &BuildWorkflowRequest {
            dat_path,
            source_path,
            destination_path: output_path.clone(),
            mode: BuildMode::ParentBundles,
            dry_run: false,
            strict: false,
        },
    )?;

    assert_eq!(report.exit_code, 0);
    assert_eq!(report.build_report.matched_roms, 2);
    assert_eq!(report.written_paths, vec![output_path.join("parent.zip")]);
    Ok(())
}

#[test]
fn strict_run_workflow_writes_nothing_when_roms_are_missing()
-> Result<(), Box<dyn std::error::Error>> {
    let pool = in_memory_pool()?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = write_clone_dat(work_dir.path())?;
    let source_path = write_present_clone_roms(source_dir.path())?;
    let output_path = utf8_path(output_dir.path())?.join("strict-output");

    let report = app::run(
        &pool,
        &RunWorkflowRequest {
            dat_path,
            source_path,
            destination_path: output_path.clone(),
            mode: BuildMode::ParentBundles,
            jobs: 1,
            dry_run: false,
            strict: true,
        },
    )?;

    assert_eq!(report.exit_code, 2);
    assert_eq!(report.build_report.matched_roms, 2);
    assert_eq!(report.build_report.missing_roms.len(), 1);
    assert_eq!(report.build_report.missing_roms[0].rom_name, "clone1.rom");
    assert!(report.written_paths.is_empty());
    assert!(!output_path.exists());
    Ok(())
}

#[test]
fn overlapping_dats_with_same_game_and_rom_names_build_independently()
-> Result<(), Box<dyn std::error::Error>> {
    let pool = in_memory_pool()?;
    let dat_dir = tempfile::tempdir()?;
    let abc_source_dir = tempfile::tempdir()?;
    let empty_source_dir = tempfile::tempdir()?;
    let abc_output_dir = tempfile::tempdir()?;
    let empty_output_dir = tempfile::tempdir()?;
    let dat_a = write_shared_dat(
        dat_dir.path(),
        "set-a.dat",
        "Set A",
        "a9993e364706816aba3e25717850c26c9cd0d89d",
    )?;
    let dat_b = write_shared_dat(
        dat_dir.path(),
        "set-b.dat",
        "Set B",
        "da39a3ee5e6b4b0d3255bfef95601890afd80709",
    )?;
    let source_a = write_single_rom_source(abc_source_dir.path(), b"abc")?;
    let source_b = write_single_rom_source(empty_source_dir.path(), b"")?;
    let output_a = utf8_path(abc_output_dir.path())?.to_path_buf();
    let output_b = utf8_path(empty_output_dir.path())?.to_path_buf();

    app::import_dat(
        &pool,
        &DatImportRequest {
            dat_path: dat_a.clone(),
        },
    )?;
    app::import_dat(
        &pool,
        &DatImportRequest {
            dat_path: dat_b.clone(),
        },
    )?;
    app::scan_source(
        &pool,
        &SourceScanRequest {
            source_path: source_a.clone(),
            jobs: 1,
        },
    )?;
    app::scan_source(
        &pool,
        &SourceScanRequest {
            source_path: source_b.clone(),
            jobs: 1,
        },
    )?;

    let report_a = app::build(
        &pool,
        &BuildWorkflowRequest {
            dat_path: dat_a,
            source_path: source_a,
            destination_path: output_a.clone(),
            mode: BuildMode::ParentBundles,
            dry_run: false,
            strict: true,
        },
    )?;
    let report_b = app::build(
        &pool,
        &BuildWorkflowRequest {
            dat_path: dat_b,
            source_path: source_b,
            destination_path: output_b.clone(),
            mode: BuildMode::ParentBundles,
            dry_run: false,
            strict: true,
        },
    )?;

    assert_eq!(report_a.exit_code, 0);
    assert_eq!(report_b.exit_code, 0);
    assert_eq!(report_a.build_report.matched_roms, 1);
    assert_eq!(report_b.build_report.matched_roms, 1);
    assert_eq!(
        zip_entries(&output_a.join("shared.zip"))?
            .get("shared.rom")
            .map(Vec::as_slice),
        Some(b"abc" as &[u8])
    );
    assert_eq!(
        zip_entries(&output_b.join("shared.zip"))?
            .get("shared.rom")
            .map(Vec::as_slice),
        Some(b"" as &[u8])
    );
    Ok(())
}
