use camino::Utf8PathBuf;
use mame_coalesce::app::{self, CatalogDocumentFormat, CatalogImportRequest};
use mame_coalesce::catalog_files::{DigestProvenance, occurrences_for_ids};
use mame_coalesce::catalog_logiqx::{LogiqxPageLimit, logiqx_for_snapshot};
use mame_coalesce::database::Database;
use mame_coalesce::domain::{CatalogKey, CatalogScope, PublishingSourceKey};
use mame_coalesce::logiqx::LogiqxMode;
use std::path::Path;
use tempfile::TempDir;

use super::{PAGE_SIZE, VerifyResult};

const HEADER: &str = r#"<header><name>Catalog</name><description>Fixture</description><version>1</version><author>Test</author></header>"#;

struct Published {
    _directory: TempDir,
    database_path: Utf8PathBuf,
    database: Database,
    source_path: Utf8PathBuf,
    snapshot: mame_coalesce::domain::SnapshotKey,
    mode: LogiqxMode,
}

impl Published {
    fn new(xml: &str, mode: LogiqxMode, key: &str) -> VerifyResult<Self> {
        let directory = tempfile::tempdir()?;
        let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        let source_path = Utf8PathBuf::try_from(directory.path().join("source.xml"))?;
        std::fs::write(&source_path, xml)?;
        let database = Database::open(&database_path)?;
        let report = app::import_catalog(
            &database,
            &CatalogImportRequest {
                document_path: source_path.clone(),
                format: CatalogDocumentFormat::Logiqx(mode),
                source_key: PublishingSourceKey::new(format!("verify-source-{key}")),
                source_display_name: format!("Verifier source {key}"),
                catalog_key: CatalogKey::new(format!("verify-catalog-{key}")),
                catalog_display_name: format!("Verifier catalog {key}"),
                scope: CatalogScope::Complete,
            },
        )?;
        assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
        let snapshot = report
            .snapshot_key
            .ok_or("successful import omitted snapshot")?;
        Ok(Self {
            _directory: directory,
            database_path,
            database,
            source_path,
            snapshot,
            mode,
        })
    }

    fn verify_same(&self) -> VerifyResult {
        super::verify_path(
            &self.database_path,
            &self.snapshot,
            Path::new(self.source_path.as_str()),
            self.mode,
        )
    }

    fn verify_xml(&self, xml: &str) -> VerifyResult {
        let alternate = self._directory.path().join("alternate.xml");
        std::fs::write(&alternate, xml)?;
        super::verify_path(&self.database_path, &self.snapshot, &alternate, self.mode)
    }
}

fn single_game(game: &str) -> String {
    format!("<datafile build='b'>{HEADER}{game}</datafile>")
}

#[test]
fn zero_game_document_is_verified_without_inventing_a_game() -> VerifyResult {
    let fixture = Published::new(
        &format!("<datafile>{HEADER}</datafile>"),
        LogiqxMode::ObservedCompatible,
        "zero",
    )?;
    fixture.verify_same()
}

#[test]
fn compatible_text_field_reordering_is_verified_in_source_order() -> VerifyResult {
    let xml = single_game(
        "<game name='g'><year>1990</year><manufacturer>Maker</manufacturer><description>Game</description><rom name='r' size='1'/></game>",
    );
    Published::new(&xml, LogiqxMode::ObservedCompatible, "text-order")?.verify_same()
}

#[test]
fn requested_reading_rules_must_match_the_published_contract() -> VerifyResult {
    let xml = format!(
        "<datafile>{HEADER}<game name='g'><description>Game</description></game></datafile>"
    );
    for (stored, requested) in [
        (LogiqxMode::ObservedCompatible, LogiqxMode::StrictDtd15),
        (LogiqxMode::StrictDtd15, LogiqxMode::ObservedCompatible),
    ] {
        let fixture = Published::new(&xml, stored, "wrong-rules")?;
        fixture.verify_same()?;
        let error = super::verify_path(
            &fixture.database_path,
            &fixture.snapshot,
            Path::new(fixture.source_path.as_str()),
            requested,
        )
        .err()
        .ok_or("wrong reading rules unexpectedly accepted")?
        .to_string();
        assert!(
            error.contains("snapshot.interpretation"),
            "wrong failure: {error}"
        );
    }
    Ok(())
}

#[test]
fn all_document_header_game_media_defaults_presence_and_mixed_order_match() -> VerifyResult {
    let xml = r#"<datafile build="" debug="no"><file_name>declared.dat</file_name><sha1>0123456789012345678901234567890123456789</sha1>
      <header><name>Catalog</name><category>Arcade</category><description>Fixture</description><version>1</version><author>Test</author><clrmamepro header="" forcemerging="full" forcenodump="ignore" forcepacking="unzip"/><date>2026</date><email>author@example.test</email><homepage>https://example.test</homepage><url>https://example.test/dat</url><comment>header note</comment><romcenter plugin="" rommode="unmerged" biosmode="merged" samplemode="unmerged" lockrommode="yes" lockbiosmode="no" locksamplemode="yes"/></header>
      <game name="g" sourcefile="src.zip" isbios="yes" cloneof="parent" romof="rom-parent" sampleof="sample-parent" board="board" rebuildto="rebuilt">
        <comment>note</comment><description>G</description><year>1990</year><manufacturer>Maker</manufacturer>
        <release name="World" region="world" language="en" date="1990" default="yes"/><biosset name="bios" description="BIOS" default="yes"/>
        <rom name="rom.bin" size="0009" crc="AABBCCDD" md5="900150983cd24fb0d6963f7d28e17f72" sha1="a9993e364706816aba3e25717850c26c9cd0d89d" status="verified" merge="parent.bin" date="2020" serial="R1"/>
        <device_ref name="sound"/><disk name="disk.chd" sha1="a9993e364706816aba3e25717850c26c9cd0d89d" md5="900150983cd24fb0d6963f7d28e17f72" status="baddump" merge="disk-parent"/>
        <sample name="sample.wav"/><archive name="container.zip"/>
      </game></datafile>"#;
    let fixture = Published::new(xml, LogiqxMode::ObservedCompatible, "full")?;
    fixture.verify_same()
}

#[test]
fn game_pages_cross_the_sixty_four_game_boundary_in_source_order() -> VerifyResult {
    let games = (0..PAGE_SIZE + 1)
        .map(|index| {
            format!("<game name='g{index}'><description>Game {index}</description></game>")
        })
        .collect::<String>();
    let fixture = Published::new(
        &format!("<datafile>{HEADER}{games}</datafile>"),
        LogiqxMode::ObservedCompatible,
        "page-boundary",
    )?;
    fixture.verify_same()
}

#[test]
fn all_media_batches_are_checked_including_the_last_payload() -> VerifyResult {
    let count = 2 * super::OCCURRENCE_BATCH_SIZE + 1;
    let roms = (0..count)
        .map(|index| format!("<rom name='r{index}' size='1'/>"))
        .collect::<String>();
    let xml = single_game(&format!("<game name='g'>{roms}</game>"));
    let fixture = Published::new(&xml, LogiqxMode::ObservedCompatible, "media-batches")?;
    fixture.verify_same()?;
    let last_rom = format!("<rom name='r{}' size='1'/>", count - 1);
    let changed = xml.replace(&last_rom, &last_rom.replace("size='1'", "size='2'"));
    let error = fixture
        .verify_xml(&changed)
        .err()
        .ok_or("last media payload was not compared")?
        .to_string();
    assert!(
        error.contains(&format!("game.media[{}].size_text mismatch", count - 1)),
        "wrong failure: {error}"
    );
    Ok(())
}

#[test]
fn strict_dtd15_document_uses_the_same_source_to_native_projection() -> VerifyResult {
    let xml = "<datafile><header><name/><description/><version> v </version><author/></header><game name='g'><description/><rom name='g.bin' size='1' sha1='1111111111111111111111111111111111111111'/></game></datafile>";
    let fixture = Published::new(xml, LogiqxMode::StrictDtd15, "strict")?;
    fixture.verify_same()
}

#[test]
fn source_reordering_is_not_hidden_by_name_matching() -> VerifyResult {
    let fixture = Published::new(
        &format!("<datafile>{HEADER}<game name='a'/><game name='b'/></datafile>"),
        LogiqxMode::ObservedCompatible,
        "reordered",
    )?;
    assert!(
        fixture
            .verify_xml(&format!(
                "<datafile>{HEADER}<game name='b'/><game name='a'/></datafile>"
            ))
            .is_err()
    );
    Ok(())
}

#[test]
fn source_missing_final_game_fails_after_the_last_page() -> VerifyResult {
    let fixture = Published::new(
        &format!("<datafile>{HEADER}<game name='a'/><game name='b'/></datafile>"),
        LogiqxMode::ObservedCompatible,
        "missing-final",
    )?;
    assert!(
        fixture
            .verify_xml(&format!("<datafile>{HEADER}<game name='a'/></datafile>"))
            .is_err()
    );
    Ok(())
}

#[test]
fn source_extra_final_game_fails_at_eof() -> VerifyResult {
    let fixture = Published::new(
        &format!("<datafile>{HEADER}<game name='a'/></datafile>"),
        LogiqxMode::ObservedCompatible,
        "extra-final",
    )?;
    assert!(
        fixture
            .verify_xml(&format!(
                "<datafile>{HEADER}<game name='a'/><game name='extra'/></datafile>"
            ))
            .is_err()
    );
    Ok(())
}

#[test]
fn header_and_game_field_mismatches_are_reported() -> VerifyResult {
    let fixture = Published::new(
        &single_game("<game name='g'><description>G</description></game>"),
        LogiqxMode::ObservedCompatible,
        "field-mismatch",
    )?;
    assert!(
        fixture
            .verify_xml(&single_game(
                "<game name='g'><description>Changed</description></game>"
            ))
            .is_err()
    );
    assert!(fixture.verify_xml(&format!("<datafile build='b'><header><name>Wrong</name><description>Fixture</description><version>1</version><author>Test</author></header><game name='g'><description>G</description></game></datafile>")).is_err());
    Ok(())
}

#[test]
fn omitted_defaults_and_explicit_defaults_remain_distinct() -> VerifyResult {
    let imported =
        single_game("<game name='g'><description>G</description><rom name='x' size='1'/></game>");
    let fixture = Published::new(&imported, LogiqxMode::ObservedCompatible, "presence")?;
    let explicit = single_game(
        "<game name='g' isbios='no'><description>G</description><rom name='x' size='1' status='good'/></game>",
    );
    assert!(fixture.verify_xml(&explicit).is_err());
    fixture.verify_same()
}

#[test]
fn source_declared_hash_scope_and_text_are_checked_separately() -> VerifyResult {
    let hash = "a9993e364706816aba3e25717850c26c9cd0d89d";
    let fixture = Published::new(
        &single_game(&format!(
            "<game name='g'><rom name='x' size='3' sha1='{hash}'/></game>"
        )),
        LogiqxMode::ObservedCompatible,
        "scoped-hash",
    )?;
    let page = logiqx_for_snapshot(
        &fixture.database,
        &fixture.snapshot,
        None,
        LogiqxPageLimit::new(1)?,
    )?;
    let game = page.games.first().ok_or("game missing")?;
    let ids = game
        .media
        .iter()
        .map(|media| media.occurrence_id)
        .collect::<Vec<_>>();
    let files = occurrences_for_ids(&fixture.database, &ids)?;
    let digest = files
        .first()
        .and_then(|file| {
            file.digests
                .iter()
                .find(|d| d.provenance == DigestProvenance::SourceDeclared)
        })
        .ok_or("source-declared digest missing")?;
    assert_eq!(digest.scope, "whole_asset");
    fixture.verify_same()?;
    let different_spelling = single_game(
        "<game name='g'><rom name='x' size='3' sha1='A9993E364706816ABA3E25717850C26C9CD0D89D'/></game>",
    );
    assert!(fixture.verify_xml(&different_spelling).is_err());
    let wrong_hash = single_game(
        "<game name='g'><rom name='x' size='3' sha1='0000000000000000000000000000000000000000'/></game>",
    );
    assert!(fixture.verify_xml(&wrong_hash).is_err());
    Ok(())
}

#[test]
fn normalized_digest_assertions_must_match_raw_source_declarations() -> VerifyResult {
    let xml = single_game(
        "<game name='g'><rom name='x' size='3' sha1='a9993e364706816aba3e25717850c26c9cd0d89d'/></game>",
    );
    let fixture = Published::new(&xml, LogiqxMode::ObservedCompatible, "decoded-hashes")?;
    let parsed = mame_coalesce::logiqx::DataFile::from_reader(xml.as_bytes())?;
    let source_game = parsed.games().first().ok_or("source game missing")?;
    let source_rom = source_game.roms().first().ok_or("source ROM missing")?;
    let page = logiqx_for_snapshot(
        &fixture.database,
        &fixture.snapshot,
        None,
        LogiqxPageLimit::new(1)?,
    )?;
    let ids = page
        .games
        .first()
        .ok_or("native game missing")?
        .media
        .iter()
        .map(|media| media.occurrence_id)
        .collect::<Vec<_>>();
    let files = occurrences_for_ids(&fixture.database, &ids)?;
    let original = files.first().ok_or("native occurrence missing")?;
    for attack in ["missing", "bytes", "scope", "provenance", "duplicate"] {
        let mut changed = original.clone();
        match attack {
            "missing" => changed.digests.clear(),
            "bytes" => changed
                .digests
                .first_mut()
                .ok_or("digest missing")?
                .value
                .fill(0),
            "scope" => {
                changed.digests.first_mut().ok_or("digest missing")?.scope =
                    "software_segment".into()
            }
            "provenance" => {
                changed
                    .digests
                    .first_mut()
                    .ok_or("digest missing")?
                    .provenance = DigestProvenance::Computed
            }
            "duplicate" => changed
                .digests
                .push(changed.digests.first().ok_or("digest missing")?.clone()),
            _ => unreachable!(),
        }
        let Some(mame_coalesce::catalog_files::LogiqxFilePayload::Rom(payload)) =
            changed.logiqx_file.as_ref()
        else {
            return Err("native ROM payload missing".into());
        };
        let error = super::compare_rom(source_rom, source_game, payload, &changed, "rom")
            .err()
            .ok_or_else(|| format!("{attack} normalized evidence unexpectedly matched"))?
            .to_string();
        assert!(
            error.contains("declared_digests"),
            "wrong failure for {attack}: {error}"
        );
    }
    Ok(())
}

#[test]
fn malformed_tail_after_a_matching_last_game_does_not_verify_a_prefix() -> VerifyResult {
    let fixture = Published::new(
        &single_game("<game name='g'/>"),
        LogiqxMode::ObservedCompatible,
        "bad-tail",
    )?;
    assert!(
        fixture
            .verify_xml(&single_game("<game name='g'/><"))
            .is_err()
    );
    Ok(())
}

#[test]
fn same_values_at_different_source_coordinates_fail_location_proof() -> VerifyResult {
    let fixture = Published::new(
        &single_game("<game name='g'><description>G</description></game>"),
        LogiqxMode::ObservedCompatible,
        "location",
    )?;
    let moved = format!(
        "<datafile build='b'>\n{HEADER}<game name='g'><description>G</description></game></datafile>"
    );
    assert!(fixture.verify_xml(&moved).is_err());
    Ok(())
}
