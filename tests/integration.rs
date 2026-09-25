use assert_cmd::Command;
use mame_coalesce::{
    app::{
        self, BuildPlanRequest, BuildWorkflowRequest, DatImportRequest, RunWorkflowRequest,
        ScanCachePolicy, SourceScanRequest,
    },
    database::Database,
    domain::{ArtifactOutcome, BuildMode, OutputContainer, ZipCompression},
    logiqx::DataFile,
};
use predicates::str::contains;
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read, Write},
    process::Command as ProcessCommand,
};

// =====================================================================
// DAT Parsing → DB Insert
// =====================================================================

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

fn utf8_path(path: &std::path::Path) -> Result<&camino::Utf8Path, io::Error> {
    camino::Utf8Path::from_path(path).ok_or_else(|| io::Error::other("path is not UTF-8"))
}

fn test_database(path: &std::path::Path) -> Result<Database, Box<dyn std::error::Error>> {
    let database_path = path.join("test.db");
    let database_path = utf8_path(&database_path)?.to_path_buf();
    Ok(Database::open(&database_path)?)
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

fn write_version_rar(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let archive = hex::decode(
        "526172211a0700cf907300000d000000000000000f0c7420802700150000000b0000000345f37dc6a48a07471d330700a481000056455253494f4e0c008fec8a45cc23c848088362fe5fdd5c5388f072c43d7b00400700",
    )?;
    fs::write(path, archive)?;
    Ok(())
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

fn directory_entries(
    root: &std::path::Path,
) -> Result<BTreeMap<String, Vec<u8>>, Box<dyn std::error::Error>> {
    fn visit(
        root: &std::path::Path,
        current: &std::path::Path,
        entries: &mut BTreeMap<String, Vec<u8>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for entry in fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, entries)?;
            } else {
                let name = path
                    .strip_prefix(root)?
                    .to_string_lossy()
                    .replace('\\', "/");
                entries.insert(name, fs::read(path)?);
            }
        }
        Ok(())
    }

    let mut entries = BTreeMap::new();
    visit(root, root, &mut entries)?;
    Ok(entries)
}

fn cargo_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mame_coalesce"))
}

fn db_arg(path: &camino::Utf8Path) -> [&str; 2] {
    ["--cache", path.as_str()]
}

#[test]
fn one_shot_rejects_source_destination_overlap_before_catalog_import()
-> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let database = test_database(temp_dir.path())?;
    let source = temp_dir.path().join("source");
    fs::create_dir_all(&source)?;
    let source = utf8_path(&source)?.to_path_buf();
    let missing_dat = utf8_path(&temp_dir.path().join("does-not-exist.dat"))?.to_path_buf();

    let result = app::run(
        &database,
        &RunWorkflowRequest {
            dat_path: missing_dat,
            source_path: source.clone(),
            destination_path: source,
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            jobs: 1,
            dry_run: false,
            strict: false,
        },
    );
    let Err(error) = result else {
        return Err(io::Error::other(
            "overlapping paths are rejected before attempting the missing DAT",
        )
        .into());
    };

    assert!(error.to_string().contains("source/destination overlap"));
    Ok(())
}

#[test]
fn build_reports_partial_artifact_completion_when_a_later_source_disappears()
-> Result<(), Box<dyn std::error::Error>> {
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = work_dir.path().join("two-games.dat");
    fs::write(
        &dat_path,
        r#"<?xml version="1.0"?>
<datafile><header><name>Two games</name></header>
<game name="a"><rom name="a.rom" size="1" crc="e8b7be43" md5="0cc175b9c0f1b6a831c399e269772661" sha1="86f7e437faa5a7fce15d1ddcb9eaeaea377667b8"/></game>
<game name="b"><rom name="b.rom" size="1" crc="71beeff9" md5="92eb5ffee6ae2fec3ad71c777531578f" sha1="e9d71f5ee7c92d6dc9e92ffdad17b8bd49418f98"/></game>
</datafile>"#,
    )?;
    let dat_path = utf8_path(&dat_path)?.to_path_buf();
    let source_path = utf8_path(source_dir.path())?.to_path_buf();
    let output_path = utf8_path(output_dir.path())?.join("out");
    fs::write(source_dir.path().join("a.rom"), b"a")?;
    fs::write(source_dir.path().join("b.rom"), b"b")?;
    app::import_dat(
        &database,
        &DatImportRequest {
            dat_path: dat_path.clone(),
        },
    )?;
    app::scan_source(
        &database,
        &SourceScanRequest {
            source_path: source_path.clone(),
            jobs: 1,
        },
    )?;
    fs::remove_file(source_dir.path().join("b.rom"))?;

    let report = app::build(
        &database,
        &BuildWorkflowRequest {
            dat_path,
            source_path,
            destination_path: output_path.clone(),
            mode: BuildMode::PerGame,
            compression: ZipCompression::Deflate,
            dry_run: false,
            strict: false,
        },
    )?;

    assert!(
        report
            .artifact_results
            .iter()
            .any(|artifact| matches!(&artifact.outcome, ArtifactOutcome::Failed { .. }))
    );
    assert_eq!(report.written_paths, vec![output_path.join("a.zip")]);
    let mut results = report.artifact_results.iter();
    assert_eq!(
        results.next().map(|result| &result.outcome),
        Some(&ArtifactOutcome::Completed)
    );
    let failed = results
        .next()
        .ok_or_else(|| io::Error::other("expected the second artifact result"))?;
    let ArtifactOutcome::Failed { error } = &failed.outcome else {
        return Err(io::Error::other("the second artifact should report failure").into());
    };
    assert!(!error.is_empty());
    assert!(results.next().is_none());
    assert_eq!(
        zip_entries(&output_path.join("a.zip"))?
            .get("a.rom")
            .map(Vec::as_slice),
        Some(b"a" as &[u8])
    );
    Ok(())
}

// =====================================================================
// DAT → DB insert
// =====================================================================

#[test]
fn parse_and_insert_clone_dat() -> Result<(), Box<dyn std::error::Error>> {
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
    assert_eq!(clone1.cloneof(), Some("parent"));
    Ok(())
}

#[test]
fn run_workflow_writes_from_7z_archive() -> Result<(), Box<dyn std::error::Error>> {
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = write_shared_dat(
        work_dir.path(),
        "set-7z.dat",
        "Set 7z",
        "a9993e364706816aba3e25717850c26c9cd0d89d",
    )?;
    let archive_data = r7z::ArchiveBuilder::new()
        .add_file("shared.rom", b"abc")
        .build()?;
    fs::write(source_dir.path().join("source.7z"), archive_data)?;
    let source_path = utf8_path(source_dir.path())?.to_path_buf();
    let output_path = utf8_path(output_dir.path())?.to_path_buf();

    let report = app::run(
        &database,
        &RunWorkflowRequest {
            dat_path,
            source_path,
            destination_path: output_path.clone(),
            mode: BuildMode::ParentBundles,
            jobs: 1,
            compression: ZipCompression::Deflate,
            dry_run: false,
            strict: true,
        },
    )?;

    assert_eq!(
        report.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Ready
    );
    assert_eq!(report.build_report.matched_roms, 1);
    assert!(report.build_report.missing_roms.is_empty());
    assert_eq!(report.written_paths, vec![output_path.join("shared.zip")]);
    assert_eq!(
        zip_entries(&output_path.join("shared.zip"))?
            .get("shared.rom")
            .map(Vec::as_slice),
        Some(b"abc" as &[u8])
    );
    Ok(())
}

#[test]
fn run_workflow_writes_from_rar_archive() -> Result<(), Box<dyn std::error::Error>> {
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = write_shared_dat(
        work_dir.path(),
        "set-rar.dat",
        "Set RAR",
        "baffb26680d43e04ed6fcf558a8a1bb772e6b8f6",
    )?;
    write_version_rar(&source_dir.path().join("source.rar"))?;
    let source_path = utf8_path(source_dir.path())?.to_path_buf();
    let output_path = utf8_path(output_dir.path())?.to_path_buf();

    let report = app::run(
        &database,
        &RunWorkflowRequest {
            dat_path,
            source_path,
            destination_path: output_path.clone(),
            mode: BuildMode::ParentBundles,
            jobs: 1,
            compression: ZipCompression::Deflate,
            dry_run: false,
            strict: true,
        },
    )?;

    assert_eq!(
        report.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Ready
    );
    assert_eq!(report.build_report.matched_roms, 1);
    assert!(report.build_report.missing_roms.is_empty());
    assert_eq!(report.written_paths, vec![output_path.join("shared.zip")]);
    assert_eq!(
        zip_entries(&output_path.join("shared.zip"))?
            .get("shared.rom")
            .map(Vec::as_slice),
        Some(b"unrar-0.4.0" as &[u8])
    );
    Ok(())
}

#[test]
fn p7zip_extracts_r7z_builder_archive() -> Result<(), Box<dyn std::error::Error>> {
    let work_dir = tempfile::tempdir()?;
    let archive_path = work_dir.path().join("source.7z");
    let extract_dir = work_dir.path().join("extract");
    let archive_data = r7z::ArchiveBuilder::new()
        .add_file("nested/shared.rom", b"abc")
        .build()?;
    fs::write(&archive_path, archive_data)?;

    let output = ProcessCommand::new("7z")
        .arg("x")
        .arg(&archive_path)
        .arg(format!("-o{}", extract_dir.display()))
        .arg("-y")
        .output()?;

    assert!(
        output.status.success(),
        "7z failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(extract_dir.join("nested").join("shared.rom"))?,
        b"abc"
    );
    Ok(())
}

#[test]
fn importing_dat_links_previously_scanned_rom_files() -> Result<(), Box<dyn std::error::Error>> {
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = write_clone_dat(work_dir.path())?;
    let source_path = write_present_clone_roms(source_dir.path())?;
    let output_path = utf8_path(output_dir.path())?.to_path_buf();

    app::scan_source(
        &database,
        &SourceScanRequest {
            source_path: source_path.clone(),
            jobs: 1,
        },
    )?;
    app::import_dat(
        &database,
        &DatImportRequest {
            dat_path: dat_path.clone(),
        },
    )?;

    let report = app::build(
        &database,
        &BuildWorkflowRequest {
            dat_path,
            source_path,
            destination_path: output_path,
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            dry_run: false,
            strict: false,
        },
    )?;

    assert_eq!(report.build_report.matched_roms, 2);
    Ok(())
}

#[test]
fn run_workflow_writes_parent_bundle_zip() -> Result<(), Box<dyn std::error::Error>> {
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = write_clone_dat(work_dir.path())?;
    let source_path = write_present_clone_roms(source_dir.path())?;
    let output_path = utf8_path(output_dir.path())?.to_path_buf();

    let report = app::run(
        &database,
        &RunWorkflowRequest {
            dat_path,
            source_path,
            destination_path: output_path.clone(),
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            jobs: 1,
            dry_run: false,
            strict: false,
        },
    )?;

    assert_eq!(
        report.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Ready
    );
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
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = write_clone_dat(work_dir.path())?;
    let source_path = write_present_clone_roms(source_dir.path())?;
    let output_path = utf8_path(output_dir.path())?.join("dry-run-output");

    let report = app::run(
        &database,
        &RunWorkflowRequest {
            dat_path,
            source_path,
            destination_path: output_path.clone(),
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            jobs: 1,
            dry_run: true,
            strict: false,
        },
    )?;

    assert_eq!(
        report.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Ready
    );
    assert!(
        report
            .artifact_results
            .iter()
            .all(|artifact| artifact.outcome == ArtifactOutcome::Unattempted)
    );
    assert_eq!(report.build_report.matched_roms, 2);
    assert_eq!(report.build_report.missing_roms.len(), 1);
    assert!(report.written_paths.is_empty());
    assert!(!output_path.exists());
    Ok(())
}

#[test]
fn build_workflow_accepts_imported_dat_name() -> Result<(), Box<dyn std::error::Error>> {
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = write_clone_dat(work_dir.path())?;
    let source_path = write_present_clone_roms(source_dir.path())?;
    let output_path = utf8_path(output_dir.path())?.to_path_buf();

    app::import_dat(&database, &DatImportRequest { dat_path })?;
    app::scan_source(
        &database,
        &SourceScanRequest {
            source_path: source_path.clone(),
            jobs: 1,
        },
    )?;

    let report = app::build(
        &database,
        &BuildWorkflowRequest {
            dat_path: camino::Utf8PathBuf::from("Clone Test"),
            source_path,
            destination_path: output_path.clone(),
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            dry_run: false,
            strict: false,
        },
    )?;

    assert_eq!(
        report.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Ready
    );
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
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = write_clone_dat(work_dir.path())?;
    let source_path = write_present_clone_roms(source_dir.path())?;
    let scanned_source_path = path_with_parent_component(&source_path)?;
    let output_path = utf8_path(output_dir.path())?.to_path_buf();

    app::import_dat(
        &database,
        &DatImportRequest {
            dat_path: dat_path.clone(),
        },
    )?;
    let scan_report = app::scan_source(
        &database,
        &SourceScanRequest {
            source_path: scanned_source_path,
            jobs: 1,
        },
    )?;

    assert_eq!(scan_report.source_path, source_path.canonicalize_utf8()?);

    let report = app::build(
        &database,
        &BuildWorkflowRequest {
            dat_path,
            source_path,
            destination_path: output_path.clone(),
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            dry_run: false,
            strict: false,
        },
    )?;

    assert_eq!(
        report.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Ready
    );
    assert_eq!(report.build_report.matched_roms, 2);
    assert_eq!(report.written_paths, vec![output_path.join("parent.zip")]);
    Ok(())
}

#[test]
fn source_scan_replaces_rows_for_source_root() -> Result<(), Box<dyn std::error::Error>> {
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dirs = [tempfile::tempdir()?, tempfile::tempdir()?];
    let dat_a = write_single_game_dat(
        &work_dir.path().join("set-a.dat"),
        "Refresh Set A",
        "game-a",
        "a.rom",
        "a9993e364706816aba3e25717850c26c9cd0d89d",
    )?;
    let dat_b = write_single_game_dat(
        &work_dir.path().join("set-b.dat"),
        "Refresh Set B",
        "game-b",
        "b.rom",
        "da39a3ee5e6b4b0d3255bfef95601890afd80709",
    )?;
    let source_path = utf8_path(source_dir.path())?.to_path_buf();

    app::import_dat(
        &database,
        &DatImportRequest {
            dat_path: dat_a.clone(),
        },
    )?;
    app::import_dat(
        &database,
        &DatImportRequest {
            dat_path: dat_b.clone(),
        },
    )?;
    fs::write(source_dir.path().join("a.rom"), b"abc")?;
    app::scan_source(
        &database,
        &SourceScanRequest {
            source_path: source_path.clone(),
            jobs: 1,
        },
    )?;

    fs::remove_file(source_dir.path().join("a.rom"))?;
    fs::write(source_dir.path().join("b.rom"), b"")?;
    app::scan_source(
        &database,
        &SourceScanRequest {
            source_path: source_path.clone(),
            jobs: 1,
        },
    )?;

    let stale_report = app::build(
        &database,
        &BuildWorkflowRequest {
            dat_path: dat_a,
            source_path: source_path.clone(),
            destination_path: utf8_path(output_dirs[0].path())?.to_path_buf(),
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            dry_run: false,
            strict: true,
        },
    )?;
    let fresh_report = app::build(
        &database,
        &BuildWorkflowRequest {
            dat_path: dat_b,
            source_path,
            destination_path: utf8_path(output_dirs[1].path())?.to_path_buf(),
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            dry_run: false,
            strict: true,
        },
    )?;

    assert!(matches!(
        stale_report.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Blocked(_)
    ));
    assert_eq!(stale_report.build_report.missing_roms.len(), 1);
    assert_eq!(
        fresh_report.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Ready
    );
    assert_eq!(fresh_report.build_report.matched_roms, 1);
    Ok(())
}

#[cfg(unix)]
#[test]
fn opt_in_bare_file_reuse_supports_forced_rehash_and_detects_same_mtime_changes()
-> Result<(), Box<dyn std::error::Error>> {
    use std::fs::FileTimes;

    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let source_dir = tempfile::tempdir()?;
    let source_path = utf8_path(source_dir.path())?.to_path_buf();
    let rom_path = source_dir.path().join("a.rom");
    fs::write(&rom_path, b"abc")?;
    let scan_request = SourceScanRequest {
        source_path: source_path.clone(),
        jobs: 1,
    };

    app::scan_source(&database, &scan_request)?;
    let reuse_unchanged = ScanCachePolicy::ReuseUnchangedBareFiles {
        force_rehash: Vec::new(),
    };
    let reused = app::scan_source_with_policy(&database, &scan_request, reuse_unchanged.clone())?;
    assert_eq!(reused.reused_bare_files, 1);

    let forced = app::scan_source_with_policy(
        &database,
        &scan_request,
        ScanCachePolicy::ReuseUnchangedBareFiles {
            force_rehash: vec![utf8_path(&rom_path)?.to_path_buf()],
        },
    )?;
    assert_eq!(forced.reused_bare_files, 0);

    let original_mtime = fs::metadata(&rom_path)?.modified()?;
    fs::write(&rom_path, b"xyz")?;
    fs::File::options()
        .write(true)
        .open(&rom_path)?
        .set_times(FileTimes::new().set_modified(original_mtime))?;
    assert_eq!(fs::metadata(&rom_path)?.modified()?, original_mtime);

    let changed = app::scan_source_with_policy(&database, &scan_request, reuse_unchanged.clone())?;
    assert_eq!(changed.reused_bare_files, 0);
    let next = app::scan_source_with_policy(&database, &scan_request, reuse_unchanged)?;
    assert_eq!(next.reused_bare_files, 1);

    let work_dir = tempfile::tempdir()?;
    let dat_path = write_single_game_dat(
        &work_dir.path().join("new-content.dat"),
        "New content after same-mtime edit",
        "game",
        "a.rom",
        &hex::encode(mame_coalesce::hashes::sha1_bytes(b"xyz")),
    )?;
    app::import_dat(
        &database,
        &DatImportRequest {
            dat_path: dat_path.clone(),
        },
    )?;
    let plan = app::plan_build(
        &database,
        &BuildPlanRequest {
            dat_path,
            source_path,
            mode: BuildMode::ParentBundles,
            matching_policy: mame_coalesce::domain::MatchingPolicy::Sha1Compatibility,
            missing_policy: mame_coalesce::domain::MissingContentPolicy::RequireComplete,
        },
    )?;
    assert_eq!(plan.report.matched_roms, 1);
    assert!(plan.report.missing_roms.is_empty());
    Ok(())
}

#[test]
fn parent_refresh_removes_stale_observations_owned_by_overlapping_root()
-> Result<(), Box<dyn std::error::Error>> {
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let nested_dir = source_dir.path().join("nested");
    fs::create_dir(&nested_dir)?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = write_single_game_dat(
        &work_dir.path().join("overlap.dat"),
        "Overlapping root refresh",
        "game",
        "nested.rom",
        "a9993e364706816aba3e25717850c26c9cd0d89d",
    )?;
    let parent_root = utf8_path(source_dir.path())?.to_path_buf();
    let child_root = utf8_path(&nested_dir)?.to_path_buf();
    fs::write(nested_dir.join("nested.rom"), b"abc")?;
    app::import_dat(
        &database,
        &DatImportRequest {
            dat_path: dat_path.clone(),
        },
    )?;

    for source_path in [&parent_root, &child_root] {
        app::scan_source(
            &database,
            &SourceScanRequest {
                source_path: source_path.clone(),
                jobs: 1,
            },
        )?;
    }
    fs::remove_file(nested_dir.join("nested.rom"))?;
    app::scan_source(
        &database,
        &SourceScanRequest {
            source_path: parent_root.clone(),
            jobs: 1,
        },
    )?;

    let report = app::build(
        &database,
        &BuildWorkflowRequest {
            dat_path,
            source_path: parent_root,
            destination_path: utf8_path(output_dir.path())?.to_path_buf(),
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            dry_run: true,
            strict: true,
        },
    )?;
    assert!(matches!(
        report.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Blocked(_)
    ));
    assert_eq!(report.build_report.missing_roms.len(), 1);
    Ok(())
}

#[test]
fn failed_archive_scan_preserves_cached_source_rows() -> Result<(), Box<dyn std::error::Error>> {
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = write_single_game_dat(
        &work_dir.path().join("archive-error.dat"),
        "Archive read failure",
        "game",
        "a.rom",
        "a9993e364706816aba3e25717850c26c9cd0d89d",
    )?;
    let source_path = utf8_path(source_dir.path())?.to_path_buf();
    fs::write(source_dir.path().join("a.rom"), b"abc")?;
    app::import_dat(
        &database,
        &DatImportRequest {
            dat_path: dat_path.clone(),
        },
    )?;
    app::scan_source(
        &database,
        &SourceScanRequest {
            source_path: source_path.clone(),
            jobs: 1,
        },
    )?;

    fs::remove_file(source_dir.path().join("a.rom"))?;
    fs::write(
        source_dir.path().join("broken.zip"),
        b"PK\x03\x04not a zip archive",
    )?;
    let Err(error) = app::scan_source(
        &database,
        &SourceScanRequest {
            source_path: source_path.clone(),
            jobs: 1,
        },
    ) else {
        return Err("expected corrupt archive to fail the scan".into());
    };
    assert!(error.to_string().contains("ZIP"), "{error}");

    let report = app::build(
        &database,
        &BuildWorkflowRequest {
            dat_path,
            source_path,
            destination_path: utf8_path(output_dir.path())?.to_path_buf(),
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            dry_run: true,
            strict: true,
        },
    )?;
    assert_eq!(
        report.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Ready
    );
    assert_eq!(report.build_report.matched_roms, 1);
    Ok(())
}

#[test]
fn source_scan_does_not_delete_similarly_prefixed_root() -> Result<(), Box<dyn std::error::Error>> {
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let work_dir = tempfile::tempdir()?;
    let dat_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let source_dir = work_dir.path().join("source");
    let prefixed_source_dir = work_dir.path().join("source-other");
    fs::create_dir(&source_dir)?;
    fs::create_dir(&prefixed_source_dir)?;
    fs::write(source_dir.join("a.rom"), b"abc")?;
    fs::write(prefixed_source_dir.join("b.rom"), b"")?;
    let dat_a = write_single_game_dat(
        &dat_dir.path().join("set-a.dat"),
        "Boundary Set A",
        "game-a",
        "a.rom",
        "a9993e364706816aba3e25717850c26c9cd0d89d",
    )?;
    let dat_b = write_single_game_dat(
        &dat_dir.path().join("set-b.dat"),
        "Boundary Set B",
        "game-b",
        "b.rom",
        "da39a3ee5e6b4b0d3255bfef95601890afd80709",
    )?;
    let source_path = utf8_path(&source_dir)?.to_path_buf();
    let prefixed_source_path = utf8_path(&prefixed_source_dir)?.to_path_buf();

    app::import_dat(
        &database,
        &DatImportRequest {
            dat_path: dat_a.clone(),
        },
    )?;
    app::import_dat(&database, &DatImportRequest { dat_path: dat_b })?;
    app::scan_source(
        &database,
        &SourceScanRequest {
            source_path: source_path.clone(),
            jobs: 1,
        },
    )?;
    app::scan_source(
        &database,
        &SourceScanRequest {
            source_path: prefixed_source_path,
            jobs: 1,
        },
    )?;

    let report = app::build(
        &database,
        &BuildWorkflowRequest {
            dat_path: dat_a,
            source_path,
            destination_path: utf8_path(output_dir.path())?.to_path_buf(),
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            dry_run: false,
            strict: true,
        },
    )?;

    assert_eq!(
        report.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Ready
    );
    assert_eq!(report.build_report.matched_roms, 1);
    Ok(())
}

#[test]
fn one_shot_build_does_not_use_stale_removed_source_file() -> Result<(), Box<dyn std::error::Error>>
{
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dirs = [tempfile::tempdir()?, tempfile::tempdir()?];
    let dat_path = write_single_game_dat(
        &work_dir.path().join("set-a.dat"),
        "One Shot Stale",
        "game-a",
        "a.rom",
        "a9993e364706816aba3e25717850c26c9cd0d89d",
    )?;
    let source_path = utf8_path(source_dir.path())?.to_path_buf();
    fs::write(source_dir.path().join("a.rom"), b"abc")?;

    let first_report = app::run(
        &database,
        &RunWorkflowRequest {
            dat_path: dat_path.clone(),
            source_path: source_path.clone(),
            destination_path: utf8_path(output_dirs[0].path())?.to_path_buf(),
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            jobs: 1,
            dry_run: false,
            strict: true,
        },
    )?;
    fs::remove_file(source_dir.path().join("a.rom"))?;
    let second_report = app::run(
        &database,
        &RunWorkflowRequest {
            dat_path,
            source_path,
            destination_path: utf8_path(output_dirs[1].path())?.to_path_buf(),
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            jobs: 1,
            dry_run: false,
            strict: true,
        },
    )?;

    assert_eq!(
        first_report.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Ready
    );
    assert!(matches!(
        second_report.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Blocked(_)
    ));
    assert!(second_report.written_paths.is_empty());
    Ok(())
}

#[test]
fn strict_run_workflow_writes_nothing_when_roms_are_missing()
-> Result<(), Box<dyn std::error::Error>> {
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = write_clone_dat(work_dir.path())?;
    let source_path = write_present_clone_roms(source_dir.path())?;
    let output_path = utf8_path(output_dir.path())?.join("strict-output");

    let report = app::run(
        &database,
        &RunWorkflowRequest {
            dat_path,
            source_path,
            destination_path: output_path.clone(),
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            jobs: 1,
            dry_run: false,
            strict: true,
        },
    )?;

    assert!(matches!(
        report.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Blocked(_)
    ));
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
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
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
        &database,
        &DatImportRequest {
            dat_path: dat_a.clone(),
        },
    )?;
    app::import_dat(
        &database,
        &DatImportRequest {
            dat_path: dat_b.clone(),
        },
    )?;
    app::scan_source(
        &database,
        &SourceScanRequest {
            source_path: source_a.clone(),
            jobs: 1,
        },
    )?;
    app::scan_source(
        &database,
        &SourceScanRequest {
            source_path: source_b.clone(),
            jobs: 1,
        },
    )?;

    let report_a = app::build(
        &database,
        &BuildWorkflowRequest {
            dat_path: dat_a,
            source_path: source_a,
            destination_path: output_a.clone(),
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            dry_run: false,
            strict: true,
        },
    )?;
    let report_b = app::build(
        &database,
        &BuildWorkflowRequest {
            dat_path: dat_b,
            source_path: source_b,
            destination_path: output_b.clone(),
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            dry_run: false,
            strict: true,
        },
    )?;

    assert_eq!(
        report_a.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Ready
    );
    assert_eq!(
        report_b.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Ready
    );
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

#[test]
fn one_scanned_source_file_can_build_matching_roms_from_multiple_dats()
-> Result<(), Box<dyn std::error::Error>> {
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let dat_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dirs = [tempfile::tempdir()?, tempfile::tempdir()?];
    let shared_sha1 = "a9993e364706816aba3e25717850c26c9cd0d89d";
    let dat_a = write_single_game_dat(
        &dat_dir.path().join("set-a.dat"),
        "Set A Shared Source",
        "game-a",
        "a.rom",
        shared_sha1,
    )?;
    let dat_b = write_single_game_dat(
        &dat_dir.path().join("set-b.dat"),
        "Set B Shared Source",
        "game-b",
        "b.rom",
        shared_sha1,
    )?;
    let source_path = write_single_rom_source(source_dir.path(), b"abc")?;
    let output_a = utf8_path(output_dirs[0].path())?.to_path_buf();
    let output_b = utf8_path(output_dirs[1].path())?.to_path_buf();

    app::import_dat(
        &database,
        &DatImportRequest {
            dat_path: dat_a.clone(),
        },
    )?;
    app::import_dat(
        &database,
        &DatImportRequest {
            dat_path: dat_b.clone(),
        },
    )?;
    app::scan_source(
        &database,
        &SourceScanRequest {
            source_path: source_path.clone(),
            jobs: 1,
        },
    )?;

    let report_a = app::build(
        &database,
        &BuildWorkflowRequest {
            dat_path: dat_a,
            source_path: source_path.clone(),
            destination_path: output_a.clone(),
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            dry_run: false,
            strict: true,
        },
    )?;
    let report_b = app::build(
        &database,
        &BuildWorkflowRequest {
            dat_path: dat_b,
            source_path,
            destination_path: output_b.clone(),
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            dry_run: false,
            strict: true,
        },
    )?;

    assert_eq!(
        report_a.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Ready
    );
    assert_eq!(
        report_b.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Ready
    );
    assert_eq!(report_a.build_report.matched_roms, 1);
    assert_eq!(report_b.build_report.matched_roms, 1);
    assert_eq!(
        zip_entries(&output_a.join("game-a.zip"))?
            .get("a.rom")
            .map(Vec::as_slice),
        Some(b"abc" as &[u8])
    );
    assert_eq!(
        zip_entries(&output_b.join("game-b.zip"))?
            .get("b.rom")
            .map(Vec::as_slice),
        Some(b"abc" as &[u8])
    );
    Ok(())
}

#[test]
fn cli_help_commands_render_successfully() {
    for args in [
        vec!["--help"],
        vec!["build", "--help"],
        vec!["cache", "--help"],
        vec!["cache", "import", "--help"],
        vec!["cache", "scan", "--help"],
        vec!["cache", "build", "--help"],
    ] {
        cargo_command()
            .args(args)
            .assert()
            .success()
            .stdout(contains("Usage"));
    }
    cargo_command()
        .args(["cache", "scan", "--help"])
        .assert()
        .success()
        .stdout(contains("--reuse-unchanged"))
        .stdout(contains("--force-rehash"));
}

#[test]
fn cli_build_dry_run_exits_zero_and_writes_no_files() -> Result<(), Box<dyn std::error::Error>> {
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let cached_output_dir = tempfile::tempdir()?;
    let root = utf8_path(work_dir.path())?;
    let dat_path = write_clone_dat(work_dir.path())?;
    let source_path = write_present_clone_roms(source_dir.path())?;
    let database_path = root.join("cli.db");
    let output_path = utf8_path(output_dir.path())?.join("dry-run-output");

    cargo_command()
        .args(db_arg(&database_path))
        .args([
            "build",
            dat_path.as_str(),
            source_path.as_str(),
            output_path.as_str(),
            "--jobs",
            "1",
            "--dry-run",
        ])
        .assert()
        .success();

    assert!(!output_path.exists());

    let cached_output_path = utf8_path(cached_output_dir.path())?.to_path_buf();
    cargo_command()
        .args(db_arg(&database_path))
        .args([
            "cache",
            "build",
            "Clone Test",
            source_path.as_str(),
            cached_output_path.as_str(),
        ])
        .assert()
        .success();
    assert!(cached_output_path.join("parent.zip").exists());
    Ok(())
}

#[test]
fn cli_build_missing_fail_exits_two_and_writes_no_files() -> Result<(), Box<dyn std::error::Error>>
{
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let cached_output_dir = tempfile::tempdir()?;
    let root = utf8_path(work_dir.path())?;
    let dat_path = write_clone_dat(work_dir.path())?;
    let source_path = write_present_clone_roms(source_dir.path())?;
    let database_path = root.join("cli.db");
    let output_path = utf8_path(output_dir.path())?.join("strict-output");

    cargo_command()
        .args(db_arg(&database_path))
        .args([
            "build",
            dat_path.as_str(),
            source_path.as_str(),
            output_path.as_str(),
            "--jobs",
            "1",
            "--missing",
            "fail",
        ])
        .assert()
        .code(2);

    assert!(!output_path.exists());

    let cached_output_path = utf8_path(cached_output_dir.path())?.to_path_buf();
    cargo_command()
        .args(db_arg(&database_path))
        .args([
            "cache",
            "build",
            "Clone Test",
            source_path.as_str(),
            cached_output_path.as_str(),
        ])
        .assert()
        .success();
    assert!(cached_output_path.join("parent.zip").exists());
    Ok(())
}

#[test]
fn cli_cache_build_accepts_dat_header_name_after_import() -> Result<(), Box<dyn std::error::Error>>
{
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let root = utf8_path(work_dir.path())?;
    let dat_path = write_clone_dat(work_dir.path())?;
    let source_path = write_present_clone_roms(source_dir.path())?;
    let database_path = root.join("cli.db");
    let output_path = utf8_path(output_dir.path())?.to_path_buf();

    cargo_command()
        .args(db_arg(&database_path))
        .args(["cache", "import", dat_path.as_str()])
        .assert()
        .success();
    cargo_command()
        .args(db_arg(&database_path))
        .args(["cache", "scan", source_path.as_str(), "--jobs", "1"])
        .assert()
        .success();
    cargo_command()
        .args(db_arg(&database_path))
        .args([
            "cache",
            "build",
            "Clone Test",
            source_path.as_str(),
            output_path.as_str(),
        ])
        .assert()
        .success();

    let archive_path = output_path.join("parent.zip");
    assert!(archive_path.exists());
    let mut archive = zip::ZipArchive::new(fs::File::open(archive_path)?)?;
    assert_eq!(
        archive.by_name("parent.rom")?.compression(),
        zip::CompressionMethod::Deflated
    );
    Ok(())
}

#[test]
fn sha1_match_ignores_inconsistent_size_crc_and_md5() -> Result<(), Box<dyn std::error::Error>> {
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = write_single_game_dat(
        &work_dir.path().join("metadata-mismatch.dat"),
        "Metadata mismatch",
        "game",
        "shared.rom",
        "a9993e364706816aba3e25717850c26c9cd0d89d",
    )?;
    let dat = fs::read_to_string(&dat_path)?
        .replace("size=\"4096\"", "size=\"9999\"")
        .replace(
            "md5=\"900150983cd24fb0d6963f7d28e17f72\"",
            "md5=\"00000000000000000000000000000000\"",
        );
    fs::write(&dat_path, dat)?;
    let source_path = write_single_rom_source(source_dir.path(), b"abc")?;

    let report = app::run(
        &database,
        &RunWorkflowRequest {
            dat_path,
            source_path,
            destination_path: utf8_path(output_dir.path())?.to_path_buf(),
            mode: BuildMode::ParentBundles,
            jobs: 1,
            compression: ZipCompression::Deflate,
            dry_run: false,
            strict: true,
        },
    )?;

    assert_eq!(
        report.build_report.outcome,
        mame_coalesce::domain::PlanOutcome::Ready
    );
    assert_eq!(report.build_report.matched_roms, 1);
    assert!(report.build_report.missing_roms.is_empty());
    Ok(())
}

#[test]
fn normalized_zip_member_scans_and_builds() -> Result<(), Box<dyn std::error::Error>> {
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = write_single_game_dat(
        &work_dir.path().join("normalized-member.dat"),
        "Normalized member",
        "game",
        "nested/shared.rom",
        "a9993e364706816aba3e25717850c26c9cd0d89d",
    )?;
    let archive_path = source_dir.path().join("source.zip");
    let archive_file = fs::File::create(&archive_path)?;
    let mut archive = zip::ZipWriter::new(archive_file);
    archive.start_file(
        "./nested/shared.rom",
        zip::write::SimpleFileOptions::default(),
    )?;
    archive.write_all(b"abc")?;
    archive.finish()?;

    let report = app::run(
        &database,
        &RunWorkflowRequest {
            dat_path,
            source_path: utf8_path(source_dir.path())?.to_path_buf(),
            destination_path: utf8_path(output_dir.path())?.to_path_buf(),
            mode: BuildMode::ParentBundles,
            jobs: 1,
            compression: ZipCompression::Deflate,
            dry_run: false,
            strict: true,
        },
    )?;

    assert_eq!(report.build_report.matched_roms, 1);
    assert_eq!(
        zip_entries(&utf8_path(output_dir.path())?.join("game.zip"))?
            .get("nested/shared.rom")
            .map(Vec::as_slice),
        Some(b"abc" as &[u8])
    );
    Ok(())
}

#[test]
fn cli_per_game_layout_writes_separate_zip_files() -> Result<(), Box<dyn std::error::Error>> {
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let root = utf8_path(work_dir.path())?;
    let dat_path = write_clone_dat(work_dir.path())?;
    let source_path = write_present_clone_roms(source_dir.path())?;
    let database_path = root.join("cli.db");
    let output_path = utf8_path(output_dir.path())?.to_path_buf();

    cargo_command()
        .args(db_arg(&database_path))
        .args([
            "build",
            dat_path.as_str(),
            source_path.as_str(),
            output_path.as_str(),
            "--jobs",
            "1",
            "--layout",
            "per-game",
        ])
        .assert()
        .success();

    assert!(output_path.join("parent.zip").exists());
    assert!(output_path.join("clone2.zip").exists());
    Ok(())
}

#[test]
fn cli_directory_output_materializes_bare_roms() -> Result<(), Box<dyn std::error::Error>> {
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let root = utf8_path(work_dir.path())?;
    let dat_path = write_shared_dat(
        work_dir.path(),
        "directory.dat",
        "Directory Output",
        "a9993e364706816aba3e25717850c26c9cd0d89d",
    )?;
    fs::write(source_dir.path().join("shared.rom"), b"abc")?;
    let source_path = utf8_path(source_dir.path())?;
    let database_path = root.join("directory.db");
    let output_path = utf8_path(output_dir.path())?.join("out");

    cargo_command()
        .args(db_arg(&database_path))
        .args([
            "build",
            dat_path.as_str(),
            source_path.as_str(),
            output_path.as_str(),
            "--jobs",
            "1",
            "--output-container",
            "directory",
        ])
        .assert()
        .success();

    assert_eq!(fs::read(output_path.join("shared/shared.rom"))?, b"abc");
    Ok(())
}

#[test]
fn cli_directory_and_zip_outputs_match_for_each_layout() -> Result<(), Box<dyn std::error::Error>> {
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let zip_dir = tempfile::tempdir()?;
    let directory_dir = tempfile::tempdir()?;
    let root = utf8_path(work_dir.path())?;
    let dat_path = write_clone_dat(work_dir.path())?;
    let source_path = write_present_clone_roms(source_dir.path())?;
    let database_path = root.join("layout-parity.db");

    for (layout, layout_arg) in [
        ("parent-bundles", "parent-bundles"),
        ("per-game", "per-game"),
    ] {
        let zip_output = utf8_path(zip_dir.path())?.join(layout);
        let directory_output = utf8_path(directory_dir.path())?.join(layout);
        for (destination, container) in [(&zip_output, "zip"), (&directory_output, "directory")] {
            cargo_command()
                .args(db_arg(&database_path))
                .args([
                    "build",
                    dat_path.as_str(),
                    source_path.as_str(),
                    destination.as_str(),
                    "--jobs",
                    "1",
                    "--layout",
                    layout_arg,
                    "--output-container",
                    container,
                ])
                .assert()
                .success();
        }

        for group in ["parent", "clone2"] {
            if layout == "parent-bundles" && group == "clone2" {
                continue;
            }
            let zip = zip_entries(&zip_output.join(format!("{group}.zip")))?;
            let directory = directory_entries(directory_output.join(group).as_std_path())?;
            assert_eq!(zip, directory, "layout {layout}, group {group}");
        }
    }
    Ok(())
}

#[test]
fn cli_directory_output_reads_zip_7z_and_rar_sources() -> Result<(), Box<dyn std::error::Error>> {
    let work_dir = tempfile::tempdir()?;
    let dat_path = write_shared_dat(
        work_dir.path(),
        "archive-directory.dat",
        "Archive Directory Output",
        "a9993e364706816aba3e25717850c26c9cd0d89d",
    )?;
    let database_path = utf8_path(work_dir.path())?.join("archive-directory.db");

    for backend in ["zip", "7z"] {
        let source_dir = tempfile::tempdir()?;
        let archive_path = source_dir.path().join(format!("source.{backend}"));
        if backend == "zip" {
            let file = fs::File::create(&archive_path)?;
            let mut zip = zip::ZipWriter::new(file);
            zip.start_file("shared.rom", zip::write::SimpleFileOptions::default())?;
            zip.write_all(b"abc")?;
            zip.finish()?;
        } else {
            let archive_data = r7z::ArchiveBuilder::new()
                .add_file("shared.rom", b"abc")
                .build()?;
            fs::write(archive_path, archive_data)?;
        }
        let output_dir = tempfile::tempdir()?;
        assert_directory_backend_build(
            &database_path,
            &dat_path,
            source_dir.path(),
            output_dir.path(),
            b"abc",
        )?;
    }

    let source_dir = tempfile::tempdir()?;
    write_version_rar(&source_dir.path().join("source.rar"))?;
    let dat_path = write_shared_dat(
        work_dir.path(),
        "rar-directory.dat",
        "RAR Directory Output",
        "baffb26680d43e04ed6fcf558a8a1bb772e6b8f6",
    )?;
    let output_dir = tempfile::tempdir()?;
    assert_directory_backend_build(
        &database_path,
        &dat_path,
        source_dir.path(),
        output_dir.path(),
        b"unrar-0.4.0",
    )?;
    Ok(())
}

#[test]
fn directory_output_preserves_existing_group_when_cached_source_is_stale()
-> Result<(), Box<dyn std::error::Error>> {
    let database_dir = tempfile::tempdir()?;
    let database = test_database(database_dir.path())?;
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let dat_path = write_shared_dat(
        work_dir.path(),
        "stale-directory.dat",
        "Stale Directory Output",
        "a9993e364706816aba3e25717850c26c9cd0d89d",
    )?;
    let source_path = write_single_rom_source(source_dir.path(), b"abc")?;
    let output_path = utf8_path(output_dir.path())?.join("out");
    let selection = app::SourceRootSelection::single(source_path.clone());
    let request = RunWorkflowRequest {
        dat_path: dat_path.clone(),
        source_path: source_path.clone(),
        destination_path: output_path.clone(),
        mode: BuildMode::ParentBundles,
        compression: ZipCompression::Deflate,
        jobs: 1,
        dry_run: false,
        strict: true,
    };

    let initial = app::run_with_roots_and_container_and_progress(
        &database,
        &request,
        &selection,
        OutputContainer::Directory,
        &|_| {},
    )?;
    assert_eq!(initial.written_paths, vec![output_path.join("shared")]);
    fs::write(source_dir.path().join("shared.rom"), b"changed")?;

    let stale = app::build_with_roots_and_container(
        &database,
        &BuildWorkflowRequest {
            dat_path,
            source_path,
            destination_path: output_path.clone(),
            mode: BuildMode::ParentBundles,
            compression: ZipCompression::Deflate,
            dry_run: false,
            strict: true,
        },
        &selection,
        OutputContainer::Directory,
    )?;

    assert!(matches!(
        stale.artifact_results[0].outcome,
        ArtifactOutcome::Failed { .. }
    ));
    assert_eq!(fs::read(output_path.join("shared/shared.rom"))?, b"abc");
    Ok(())
}

fn assert_directory_backend_build(
    database_path: &camino::Utf8Path,
    dat_path: &camino::Utf8Path,
    source_dir: &std::path::Path,
    output_dir: &std::path::Path,
    expected: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = utf8_path(source_dir)?;
    let output_path = utf8_path(output_dir)?.join("out");
    cargo_command()
        .args(db_arg(database_path))
        .args([
            "build",
            dat_path.as_str(),
            source_path.as_str(),
            output_path.as_str(),
            "--jobs",
            "1",
            "--output-container",
            "directory",
        ])
        .assert()
        .success();
    assert_eq!(fs::read(output_path.join("shared/shared.rom"))?, expected);
    Ok(())
}

#[test]
fn cli_legacy_build_defaults_preserve_parent_bundle_deflated_zip_and_warn_missing()
-> Result<(), Box<dyn std::error::Error>> {
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let root = utf8_path(work_dir.path())?;
    let dat_path = write_clone_dat(work_dir.path())?;
    let source_path = write_present_clone_roms(source_dir.path())?;
    let database_path = root.join("cli.db");
    let output_path = utf8_path(output_dir.path())?.to_path_buf();

    cargo_command()
        .args(db_arg(&database_path))
        .args([
            "build",
            dat_path.as_str(),
            source_path.as_str(),
            output_path.as_str(),
        ])
        .assert()
        .success();

    let bundle_path = output_path.join("parent.zip");
    assert!(bundle_path.is_file());
    assert!(!output_path.join("parent").exists());

    let mut archive = zip::ZipArchive::new(fs::File::open(bundle_path)?)?;
    let mut entries = BTreeMap::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        assert_eq!(entry.compression(), zip::CompressionMethod::Deflated);
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents)?;
        entries.insert(entry.name().to_owned(), contents);
    }
    assert_eq!(
        entries.keys().map(String::as_str).collect::<Vec<_>>(),
        ["clone2.rom", "parent.rom"]
    );
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
fn cli_store_compression_writes_stored_zip_entries() -> Result<(), Box<dyn std::error::Error>> {
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let root = utf8_path(work_dir.path())?;
    let dat_path = write_clone_dat(work_dir.path())?;
    let source_path = write_present_clone_roms(source_dir.path())?;
    let database_path = root.join("cli.db");
    let output_path = utf8_path(output_dir.path())?.to_path_buf();

    cargo_command()
        .args(db_arg(&database_path))
        .args([
            "build",
            dat_path.as_str(),
            source_path.as_str(),
            output_path.as_str(),
            "--jobs",
            "1",
            "--compression",
            "store",
        ])
        .assert()
        .success();

    let file = fs::File::open(output_path.join("parent.zip"))?;
    let mut archive = zip::ZipArchive::new(file)?;
    let entry = archive.by_name("parent.rom")?;

    assert_eq!(entry.compression(), zip::CompressionMethod::Stored);
    Ok(())
}

#[test]
fn cli_invalid_dat_path_exits_one() -> Result<(), Box<dyn std::error::Error>> {
    let work_dir = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    let output_dir = tempfile::tempdir()?;
    let root = utf8_path(work_dir.path())?;
    let database_path = root.join("cli.db");
    let missing_dat = root.join("missing.dat");
    let source_path = utf8_path(source_dir.path())?;
    let output_path = utf8_path(output_dir.path())?;

    cargo_command()
        .args(db_arg(&database_path))
        .args([
            "build",
            missing_dat.as_str(),
            source_path.as_str(),
            output_path.as_str(),
            "--jobs",
            "1",
        ])
        .assert()
        .failure()
        .code(1);
    Ok(())
}
