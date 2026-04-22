use mame_coalesce::{
    db::{self, create_db_pool},
    logiqx::DataFile,
    operations,
};
use std::io;

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
    assert_eq!(clone1.cloneof().map(|s| s.as_str()), Some("parent"));
    db::traverse_and_insert_data_file(&pool, &df)?;
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
    operations::source(utf8_dir, 1, &pool)?;

    // Verify something was inserted — query rom_files table
    use diesel::prelude::*;
    use mame_coalesce::schema::rom_files::dsl::rom_files;
    let mut conn = pool.get()?;
    let count: i64 = rom_files.count().get_result(&mut conn)?;
    assert_eq!(count, 1);

    // Verify the sha1 matches
    let sha1s: Vec<Vec<u8>> = rom_files
        .select(mame_coalesce::schema::rom_files::dsl::sha1)
        .load(&mut conn)?;
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

    use diesel::prelude::*;
    use mame_coalesce::schema::rom_files::dsl::rom_files;
    let mut conn = pool.get()?;
    let count: i64 = rom_files.count().get_result(&mut conn)?;
    assert_eq!(count, 2);
    Ok(())
}
