use std::fmt::Write;

use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, connection::SimpleConnection,
    sql_query, sql_types::Text,
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_clrmamepro::{self as native, ClrMameProPageLimit, ClrMameProSetChild},
    catalog_files::{
        CatalogFileOccurrence, ClrMameProRomPayload, DigestAlgorithm, DigestProvenance,
        OccurrenceDigest, occurrences_for_ids,
    },
    clrmamepro as source,
    database::Database,
    domain::{CatalogContentId, CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
};

use super::VerifyResult;

const FULL: &str = r#"; leading lexical comment
vendor ( value "source-only" )
SeT (
  NaMe "Game 🎮é" cloneof "parent" description " Game description " year 1990
  manufacturer "Maker" rebuildto "rebuilt" sampleof "sample-parent" region ""
  releaseyear 01990 releasemonth 01 releaseday 02 serial "set-serial"
  sample "first.wav"
  RoM ( NaMe "game.bin" size "0001" crc AABBCCDD crc32 aabbccdd
    md5 900150983CD24FB0D6963F7D28E17F72 sha1 A9993E364706816ABA3E25717850C26C9CD0D89D
    merge "parent.bin" date "2026" serial "rom-serial" status good nodump baddump )
  sample "second.wav"
  vendor_set ( name "not-a-ROM" )
  rom ( name "strong.bin" size 1 sha1 3333333333333333333333333333333333333333 )
)
ClrMamePro (
  name "Catalog" description " Fixture " version "" date "2026" author "Author"
  email "author@example.test" homepage "https://example.test" url "https://example.test/dat"
  comment "not a lexical comment; retained value" category "Console" header ""
  forcemerging split forcezipping unzip forcepacking zip forcenodump ignore
  vendor_header "source-only"
)
; trailing lexical comment
"#;

struct Published {
    directory: tempfile::TempDir,
    database_path: Utf8PathBuf,
    database: Database,
    source_path: Utf8PathBuf,
    snapshot: SnapshotKey,
}

impl Published {
    fn new(contents: &str) -> VerifyResult<Self> {
        let directory = tempfile::tempdir()?;
        let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        let source_path = Utf8PathBuf::try_from(directory.path().join("source.dat"))?;
        std::fs::write(&source_path, contents)?;
        let database = Database::open(&database_path)?;
        let report = app::import_catalog(
            &database,
            &CatalogImportRequest {
                document_path: source_path.clone(),
                format: CatalogDocumentFormat::ClrMamePro,
                source_key: PublishingSourceKey::new("cmp-verifier-source"),
                source_display_name: "CMP verifier source".into(),
                catalog_key: CatalogKey::new("cmp-verifier-catalog"),
                catalog_display_name: "CMP verifier catalog".into(),
                scope: CatalogScope::Complete,
            },
        )?;
        assert_eq!(report.status, CatalogImportStatus::Succeeded);
        let snapshot = report
            .snapshot_key
            .ok_or("successful import omitted snapshot")?;
        Ok(Self {
            directory,
            database_path,
            database,
            source_path,
            snapshot,
        })
    }

    fn verify_same(&self) -> VerifyResult {
        super::verify_path(
            &self.database_path,
            &self.snapshot,
            self.source_path.as_std_path(),
        )
    }

    fn verify_source(&self, contents: &str) -> VerifyResult {
        let alternate = self.directory.path().join("alternate.dat");
        std::fs::write(&alternate, contents)?;
        super::verify_path(&self.database_path, &self.snapshot, &alternate)
    }

    fn first_rom(&self) -> VerifyResult<(ClrMameProRomPayload, CatalogFileOccurrence)> {
        let page = native::sets_for_snapshot(
            &self.database,
            &self.snapshot,
            None,
            ClrMameProPageLimit::new(1)?,
        )?;
        let rom = page
            .sets
            .first()
            .ok_or("native set missing")?
            .children
            .iter()
            .find_map(|child| match child {
                ClrMameProSetChild::Rom(rom) => Some(rom),
                _ => None,
            })
            .ok_or("native ROM missing")?;
        let file = occurrences_for_ids(&self.database, &[rom.occurrence_id])?
            .pop()
            .ok_or("native occurrence missing")?;
        Ok((rom.payload.clone(), file))
    }
}

#[test]
fn every_declared_cmp_field_matches_its_native_owner() -> VerifyResult {
    Published::new(FULL)?.verify_same()
}

#[test]
fn query_comparison_does_not_open_the_retained_original() -> VerifyResult {
    let fixture = Published::new(FULL)?;
    fixture.verify_same()?;
    std::fs::rename(
        format!("{}.documents", fixture.database_path),
        fixture
            .directory
            .path()
            .join("retained-originals-unavailable"),
    )?;
    std::fs::rename(
        &fixture.source_path,
        fixture
            .directory
            .path()
            .join("original-input-unavailable.dat"),
    )?;
    fixture.verify_source(FULL)
}

#[test]
fn changed_rom_serial_is_not_masked_by_matching_digest() -> VerifyResult {
    let fixture = Published::new(FULL)?;
    fixture.verify_same()?;
    let error = fixture
        .verify_source(&FULL.replace("rom-serial", "changed-serial"))
        .err()
        .ok_or("changed ROM serial was accepted")?
        .to_string();
    assert!(error.contains("serial"), "wrong failure: {error}");
    Ok(())
}

#[test]
fn a_late_lexical_comment_is_compared_not_ignored() -> VerifyResult {
    let fixture = Published::new(FULL)?;
    fixture.verify_same()?;
    let error = fixture
        .verify_source(&FULL.replace("trailing lexical comment", "changed lexical comment"))
        .err()
        .ok_or("changed lexical comment was accepted")?
        .to_string();
    assert!(error.contains("comment"), "wrong failure: {error}");
    Ok(())
}

#[test]
fn absent_and_present_empty_headers_are_distinct() -> VerifyResult {
    let absent = "game ( name plain )";
    let present = "clrmamepro ( version \"\" ) game ( name plain )";
    for (source, other) in [(absent, present), (present, absent)] {
        let fixture = Published::new(source)?;
        fixture.verify_same()?;
        assert!(fixture.verify_source(other).is_err());
    }
    Ok(())
}

#[test]
fn set_pages_and_unmatched_tail_are_checked() -> VerifyResult {
    let mut source = String::new();
    for index in 0..65 {
        writeln!(&mut source, "game ( name g{index} )")?;
    }
    let fixture = Published::new(&source)?;
    fixture.verify_same()?;
    assert!(
        fixture
            .verify_source(&source.replace("g64", "changed64"))
            .is_err(),
        "the set after the first 64-record page was not checked"
    );
    assert!(
        fixture
            .verify_source(&source.replace("game ( name g64 )\n", ""))
            .is_err(),
        "an unmatched native tail was accepted"
    );
    assert!(
        fixture
            .verify_source(&(source + "game ( name extra )\n"))
            .is_err(),
        "an extra source set was accepted"
    );
    Ok(())
}

#[test]
fn complete_media_children_cross_multiple_payload_batches() -> VerifyResult {
    let mut source = String::from("game ( name many\n");
    for index in 0..513 {
        writeln!(&mut source, "rom ( name r{index} serial s{index} )")?;
    }
    source.push(')');
    let fixture = Published::new(&source)?;
    fixture.verify_same()?;
    let error = fixture
        .verify_source(&source.replace("serial s512", "serial changed512"))
        .err()
        .ok_or("last media payload was not checked")?
        .to_string();
    assert!(error.contains("serial"), "wrong failure: {error}");
    Ok(())
}

#[test]
fn equal_prefix_does_not_hide_a_malformed_source_tail() -> VerifyResult {
    let fixture = Published::new("game ( name plain )")?;
    fixture.verify_same()?;
    assert!(
        fixture
            .verify_source("game ( name plain ) vendor ( value \"unterminated")
            .is_err()
    );
    Ok(())
}

struct SingleSet(Option<source::Set>);

impl source::EventConsumer for SingleSet {
    type Error = mame_coalesce::Error;

    fn consume(&mut self, event: source::Event) -> mame_coalesce::Result<()> {
        if let source::Event::Set(set) = event {
            self.0 = Some(set);
        }
        Ok(())
    }
}

#[derive(QueryableByName)]
struct Guard {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    sql: String,
}

#[test]
fn samples_accept_computed_evidence_but_reject_source_declarations() -> VerifyResult {
    let contents = "set ( name s sample x rom ( name r crc AABBCCDD ) )";
    let fixture = Published::new(contents)?;
    fixture.verify_same()?;
    let mut collector = SingleSet(None);
    let _eof = source::read_with(contents.as_bytes(), &mut collector)?;
    let source = collector.0.ok_or("source set missing")?;
    let page = native::sets_for_snapshot(
        &fixture.database,
        &fixture.snapshot,
        None,
        ClrMameProPageLimit::new(1)?,
    )?;
    let set = page.sets.first().ok_or("native set missing")?;
    let sample_id = set
        .children
        .iter()
        .find_map(|child| match child {
            ClrMameProSetChild::Sample(sample) => Some(sample.occurrence_id),
            _ => None,
        })
        .ok_or("native sample missing")?;
    let mut files = super::load_media(&fixture.database, &page)?;
    super::compare::set(&source, set, &page.snapshot, 0, &mut files)?;

    // Preserve authoritative DDL while adding a computed observation accepted
    // by the native APIs. No import-time declaration or UUID is invented.
    let mut connection = SqliteConnection::establish(fixture.database_path.as_str())?;
    let guards = sql_query("SELECT name,sql FROM sqlite_schema WHERE type='trigger' AND tbl_name='occurrence_digest_assertions' ORDER BY name")
        .load::<Guard>(&mut connection)?;
    connection.immediate_transaction::<_, diesel::result::Error, _>(|connection| {
        for guard in &guards {
            connection.batch_execute(&format!("DROP TRIGGER \"{}\"", guard.name.replace('"', "\"\"")))?;
        }
        connection.batch_execute(
            "INSERT INTO digest_values(digest_id,algorithm,digest) VALUES (90000,'sha1',zeroblob(20));
             INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance)
             SELECT MIN(occurrence_id),90000,'test_computed_scope','computed' FROM cmp_samples;"
        )?;
        for guard in &guards {
            connection.batch_execute(&guard.sql)?;
        }
        Ok(())
    })?;
    let files = super::load_media(&fixture.database, &page)?;
    let sample = files.get(&sample_id).ok_or("sample missing")?;
    assert_eq!(sample.digests.len(), 1);
    assert_eq!(
        sample
            .digests
            .first()
            .ok_or("computed sample digest missing")?
            .provenance,
        DigestProvenance::Computed
    );
    assert_eq!(sample.content_id, None);
    assert_eq!(sample.canonical_content_id, None);
    fixture.verify_same()?;

    let mut corrupted = files;
    corrupted
        .get_mut(&sample_id)
        .ok_or("sample missing")?
        .digests
        .push(OccurrenceDigest {
            algorithm: DigestAlgorithm::Sha1,
            value: vec![0; 20],
            scope: "whole_asset".into(),
            provenance: DigestProvenance::SourceDeclared,
        });
    let error = super::compare::set(&source, set, &page.snapshot, 0, &mut corrupted)
        .err()
        .ok_or("source-declared sample digest was accepted")?
        .to_string();
    assert!(error.contains("sample.digests"), "wrong failure: {error}");
    Ok(())
}

#[test]
fn ineligible_roms_reject_direct_and_canonical_uuid_links() -> VerifyResult {
    let strong = "sha1 3333333333333333333333333333333333333333";
    for declaration in [
        String::from("crc aabbccdd"),
        format!("{strong} crc aabbccdd crc32 11223344"),
        format!("{strong} nodump baddump"),
        format!("{strong} status good nodump"),
    ] {
        let contents = format!("game ( name control rom ( name control.bin {declaration} ) )");
        let fixture = Published::new(&contents)?;
        fixture.verify_same()?;
        let mut collector = SingleSet(None);
        let _eof = source::read_with(contents.as_bytes(), &mut collector)?;
        let set = collector.0.ok_or("source set missing")?;
        let asset = set.assets.first().ok_or("source ROM missing")?;
        let (rom, file) = fixture.first_rom()?;
        assert_eq!(file.content_id, None, "{declaration}");
        assert_eq!(file.canonical_content_id, None, "{declaration}");
        for canonical in [false, true] {
            let mut corrupted = file.clone();
            let link = Some(CatalogContentId::from_bytes([0x7a; 16]));
            if canonical {
                corrupted.canonical_content_id = link;
            } else {
                corrupted.content_id = link;
            }
            let error = super::compare::rom(asset, &rom, &corrupted, "rom")
                .err()
                .ok_or("ineligible UUID link was accepted")?
                .to_string();
            let field = if canonical {
                "canonical_content_id"
            } else {
                "content_id"
            };
            assert!(error.contains(field), "wrong failure: {error}");
        }
    }
    Ok(())
}

#[test]
fn lone_dump_flags_do_not_disqualify_strong_source_evidence() -> VerifyResult {
    for flag in ["nodump", "baddump"] {
        let contents = format!(
            "game ( name control rom ( name control.bin sha1 3333333333333333333333333333333333333333 {flag} ) )"
        );
        let fixture = Published::new(&contents)?;
        fixture.verify_same()?;
        let (_, file) = fixture.first_rom()?;
        assert!(
            file.content_id.is_some(),
            "fresh unambiguous {flag} evidence was not linked"
        );
        assert_eq!(file.content_id, file.canonical_content_id);
    }
    Ok(())
}

#[test]
fn raw_crc_aliases_do_not_hide_normalized_digest_corruption() -> VerifyResult {
    let fixture = Published::new(FULL)?;
    fixture.verify_same()?;
    let mut collector = SingleSet(None);
    let _eof = source::read_with(FULL.as_bytes(), &mut collector)?;
    let set = collector.0.ok_or("source set missing")?;
    let source = set.assets.first().ok_or("source ROM missing")?;
    let (rom, file) = fixture.first_rom()?;
    super::compare::rom(source, &rom, &file, "rom")?;
    assert!(!file.digests.is_empty());
    for attack in 0..5 {
        let mut corrupted = file.clone();
        match attack {
            0 => corrupted.digests.clear(),
            1 => corrupted.digests.first_mut().ok_or("digest missing")?.scope = "unknown".into(),
            2 => {
                *corrupted
                    .digests
                    .first_mut()
                    .ok_or("digest missing")?
                    .value
                    .first_mut()
                    .ok_or("digest bytes missing")? ^= 0xFF;
            }
            3 => {
                corrupted
                    .digests
                    .first_mut()
                    .ok_or("digest missing")?
                    .provenance = DigestProvenance::Computed
            }
            4 => corrupted
                .digests
                .push(corrupted.digests.first().ok_or("digest missing")?.clone()),
            _ => return Err("unexpected test attack".into()),
        }
        let error = super::compare::rom(source, &rom, &corrupted, "rom")
            .err()
            .ok_or("normalized digest attack was accepted")?
            .to_string();
        assert!(
            error.contains("rom.declared_digests"),
            "wrong failure: {error}"
        );
    }
    Ok(())
}
