use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    catalog_files::{DigestProvenance, SoftwareFilePayload, occurrences_for_ids},
    catalog_software::{self, SoftwarePageLimit},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
};
use tempfile::TempDir;

use super::{PAGE_SIZE, VerifyResult};

const FULL: &str = r#"<softwarelists build=""><softwarelist name="list" description="List">
  <software name="game" cloneof="parent" supported="partial">
    <description><![CDATA[ Game 🎮é ]]></description><year>1990</year><publisher>Maker</publisher><notes/>
    <info name="serial" value=""/><sharedfeat name="compatibility" value="yes"/>
    <part name="cart" interface="cart"><feature name="slot" value="cart"/>
      <dipswitch name="Region" tag="REGION" mask="0x03"><dipvalue name="World" value="1" default="yes"/></dipswitch>
      <dataarea name="rom" size="0x0010" width="16" endianness="big">
        <rom name="game.bin" size="0004" crc="AABBCCDD" sha1="ABCDEF0123456789ABCDEF0123456789ABCDEF01" offset="0x0000" value="00" status="baddump" loadflag="load16_word_swap"/>
        <rom name="control.bin" size="2" crc="11223344" sha1="3333333333333333333333333333333333333333" offset="4" loadflag="continue"/>
        <rom size="2" offset="6" loadflag="ignore"/>
        <rom size="2" offset="8" value="FF" loadflag="fill"/>
        <rom name="other.bin" size="2" offset="10"/>
        <rom size="2" offset="12" loadflag="reload_plain"/>
      </dataarea>
      <diskarea name="disk"><disk name="disk-image" sha1="0123456789ABCDEF0123456789ABCDEF01234567" status="good" writeable="yes"/></diskarea>
    </part>
  </software><notes>Late list notes</notes>
</softwarelist></softwarelists>"#;

struct Published {
    directory: TempDir,
    database_path: Utf8PathBuf,
    database: Database,
    source_path: Utf8PathBuf,
    snapshot: SnapshotKey,
}

impl Published {
    fn new(xml: &str) -> VerifyResult<Self> {
        Self::new_bytes(xml.as_bytes())
    }

    fn new_bytes(bytes: &[u8]) -> VerifyResult<Self> {
        let directory = tempfile::tempdir()?;
        let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        let source_path = Utf8PathBuf::try_from(directory.path().join("source.xml"))?;
        std::fs::write(&source_path, bytes)?;
        let database = Database::open(&database_path)?;
        let report = app::import_catalog(
            &database,
            &CatalogImportRequest {
                document_path: source_path.clone(),
                format: CatalogDocumentFormat::MameSoftwareListXml,
                source_key: PublishingSourceKey::new("software-verifier-source"),
                source_display_name: "Software verifier source".into(),
                catalog_key: CatalogKey::new("software-verifier-catalog"),
                catalog_display_name: "Software verifier catalog".into(),
                scope: CatalogScope::Complete,
            },
        )?;
        assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
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

    fn verify_xml(&self, xml: &str) -> VerifyResult {
        let path = self.directory.path().join("alternate.xml");
        std::fs::write(&path, xml)?;
        super::verify_path(&self.database_path, &self.snapshot, &path)
    }

    fn first_title_page(&self) -> VerifyResult<catalog_software::SoftwareTitlePage> {
        let lists = catalog_software::lists_for_snapshot(
            &self.database,
            &self.snapshot,
            SoftwarePageLimit::new(1)?,
            None,
        )?;
        let list = lists.lists.first().ok_or("missing list")?;
        Ok(catalog_software::titles_for_list(
            &self.database,
            &self.snapshot,
            list.id,
            SoftwarePageLimit::new(1)?,
            None,
        )?)
    }
}

fn item(name: &str) -> String {
    format!("<software name='{name}'><description/><year/><publisher/></software>")
}

fn list(items: &str) -> String {
    format!("<softwarelist name='list'>{items}</softwarelist>")
}

#[test]
fn every_pinned_attribute_text_default_and_operation_matches() -> VerifyResult {
    Published::new(FULL)?.verify_same()
}

#[test]
fn native_normalized_digest_corruption_is_not_hidden_by_raw_text() -> VerifyResult {
    let fixture = Published::new(FULL)?;
    let page = fixture.first_title_page()?;
    let title = page.titles.first().ok_or("missing title")?;
    let part = title.parts.first().ok_or("missing part")?;
    let area = part.areas.first().ok_or("missing area")?;
    let occurrence_id = *area.entry_ids.first().ok_or("missing occurrence")?;
    let mut files = occurrences_for_ids(&fixture.database, &[occurrence_id])?;
    let file = files.pop().ok_or("missing native occurrence")?;
    let source = mame_coalesce::mame_softwarelist::SoftwareListCatalog::parse(FULL.as_bytes())?;
    let rom = match &source.lists[0].items[0].parts[0].areas[0].components[0] {
        mame_coalesce::mame_softwarelist::SoftwareComponent::Rom(rom) => rom,
        mame_coalesce::mame_softwarelist::SoftwareComponent::Disk(_) => {
            return Err("expected ROM".into());
        }
    };
    let native = match file
        .software_file
        .as_ref()
        .ok_or("missing software payload")?
    {
        SoftwareFilePayload::Rom(rom) => rom,
        SoftwareFilePayload::Disk(_) => return Err("expected ROM payload".into()),
    };
    super::compare::rom(rom, native, &file, "rom")?;
    for attack in 0..5 {
        let mut corrupted = file.clone();
        match attack {
            0 => corrupted.digests.clear(),
            1 => corrupted.digests[0].scope = "unknown".into(),
            2 => corrupted.digests[0].value[0] ^= 0xFF,
            3 => corrupted.digests[0].provenance = DigestProvenance::Computed,
            4 => corrupted.digests.push(corrupted.digests[0].clone()),
            _ => unreachable!(),
        }
        let error = super::compare::rom(rom, native, &corrupted, "rom")
            .err()
            .ok_or("normalized digest corruption was accepted")?
            .to_string();
        assert!(
            error.contains("rom.declared_digests"),
            "wrong failure: {error}"
        );
    }
    Ok(())
}

#[test]
fn control_hashes_cannot_sneak_in_whole_file_uuid_evidence() -> VerifyResult {
    let fixture = Published::new(FULL)?;
    let page = fixture.first_title_page()?;
    let area = page
        .titles
        .first()
        .and_then(|title| title.parts.first())
        .and_then(|part| part.areas.first())
        .ok_or("missing data area")?;
    let files = occurrences_for_ids(&fixture.database, &area.entry_ids)?;
    let uuid = files
        .iter()
        .find(|file| file.provenance.occurrence_order == 0)
        .and_then(|file| file.content_id)
        .ok_or("fixture lacks eligible whole-file UUID")?;
    let mut control = files
        .into_iter()
        .find(|file| file.provenance.occurrence_order == 1)
        .ok_or("missing control entry")?;
    let source = mame_coalesce::mame_softwarelist::SoftwareListCatalog::parse(FULL.as_bytes())?;
    let rom = match &source.lists[0].items[0].parts[0].areas[0].components[1] {
        mame_coalesce::mame_softwarelist::SoftwareComponent::Rom(rom) => rom,
        mame_coalesce::mame_softwarelist::SoftwareComponent::Disk(_) => {
            return Err("expected control ROM".into());
        }
    };
    let native = match control
        .software_file
        .clone()
        .ok_or("missing software payload")?
    {
        SoftwareFilePayload::Rom(rom) => rom,
        SoftwareFilePayload::Disk(_) => return Err("expected ROM payload".into()),
    };
    super::compare::rom(rom, &native, &control, "control")?;
    for canonical in [false, true] {
        control.content_id = (!canonical).then_some(uuid);
        control.canonical_content_id = canonical.then_some(uuid);
        let error = super::compare::rom(rom, &native, &control, "control")
            .err()
            .ok_or("control hash was promoted to whole-file UUID")?
            .to_string();
        let field = if canonical {
            "control.canonical_content_uuid"
        } else {
            "control.content_uuid"
        };
        assert!(error.contains(field), "wrong failure: {error}");
    }
    Ok(())
}

#[test]
fn weak_or_absent_hashes_cannot_establish_whole_file_uuids() -> VerifyResult {
    let xml = list(
        "<software name='game'><description/><year/><publisher/><part name='cart' interface='cart'><dataarea name='rom' size='3'><rom name='strong' size='1' sha1='1111111111111111111111111111111111111111'/><rom name='weak' size='1' crc='AABBCCDD'/><rom name='unhashed' size='1'/></dataarea></part></software>",
    );
    let fixture = Published::new(&xml)?;
    fixture.verify_same()?;
    let page = fixture.first_title_page()?;
    let area = &page.titles[0].parts[0].areas[0];
    let files = occurrences_for_ids(&fixture.database, &area.entry_ids)?;
    let uuid = files[0].content_id.ok_or("strong SHA-1 lacks UUID")?;
    let source = mame_coalesce::mame_softwarelist::SoftwareListCatalog::parse(xml.as_bytes())?;
    for index in [1, 2] {
        let rom = match &source.lists[0].items[0].parts[0].areas[0].components[index] {
            mame_coalesce::mame_softwarelist::SoftwareComponent::Rom(rom) => rom,
            mame_coalesce::mame_softwarelist::SoftwareComponent::Disk(_) => {
                return Err("expected weak/unhashed ROM".into());
            }
        };
        let file = &files[index];
        let Some(SoftwareFilePayload::Rom(native)) = &file.software_file else {
            return Err("missing weak/unhashed native payload".into());
        };
        super::compare::rom(rom, native, file, "weak")?;
        for canonical in [false, true] {
            let mut corrupted = file.clone();
            corrupted.content_id = (!canonical).then_some(uuid);
            corrupted.canonical_content_id = canonical.then_some(uuid);
            let error = super::compare::rom(rom, native, &corrupted, "weak")
                .err()
                .ok_or("weak or absent hash established a whole-file UUID")?
                .to_string();
            let field = if canonical {
                "weak.canonical_content_uuid"
            } else {
                "weak.content_uuid"
            };
            assert!(error.contains(field), "wrong failure: {error}");
        }
    }
    Ok(())
}

#[test]
fn chd_header_hash_cannot_establish_a_canonical_whole_file_uuid() -> VerifyResult {
    let fixture = Published::new(FULL)?;
    fixture.verify_same()?;
    let page = fixture.first_title_page()?;
    let parts = &page.titles[0].parts[0];
    let strong = occurrences_for_ids(&fixture.database, &parts.areas[0].entry_ids)?;
    let uuid = strong[0].content_id.ok_or("strong SHA-1 lacks UUID")?;
    let mut disks = occurrences_for_ids(&fixture.database, &parts.areas[1].entry_ids)?;
    let file = disks.pop().ok_or("missing disk occurrence")?;
    let Some(SoftwareFilePayload::Disk(native)) = &file.software_file else {
        return Err("missing disk payload".into());
    };
    let source = mame_coalesce::mame_softwarelist::SoftwareListCatalog::parse(FULL.as_bytes())?;
    let disk = match &source.lists[0].items[0].parts[0].areas[1].components[0] {
        mame_coalesce::mame_softwarelist::SoftwareComponent::Disk(disk) => disk,
        mame_coalesce::mame_softwarelist::SoftwareComponent::Rom(_) => {
            return Err("expected disk".into());
        }
    };
    super::compare::disk(disk, native, &file, "disk")?;
    let mut corrupted = file.clone();
    corrupted.canonical_content_id = Some(uuid);
    let error = super::compare::disk(disk, native, &corrupted, "disk")
        .err()
        .ok_or("CHD header hash established a canonical whole-file UUID")?
        .to_string();
    assert!(
        error.contains("disk.canonical_content_uuid"),
        "wrong failure: {error}"
    );
    Ok(())
}

#[derive(QueryableByName)]
struct TriggerSql {
    #[diesel(sql_type = Text)]
    sql: String,
}

#[test]
fn orphan_occurrences_are_not_hidden_by_native_hierarchy_or_page_boundaries() -> VerifyResult {
    let repeated = (0..65).map(|_| item("same")).collect::<String>();
    for (xml, title_order) in [
        (list(&item("empty")), 0_i64),
        (FULL.to_owned(), 0),
        (list(&repeated), 64),
    ] {
        let fixture = Published::new(&xml)?;
        fixture.verify_same()?;
        let mut connection = SqliteConnection::establish(fixture.database_path.as_str())?;
        let guard = sql_query("SELECT sql FROM sqlite_schema WHERE type='trigger' AND name='occurrences_require_unpublished_snapshot'")
            .get_result::<TriggerSql>(&mut connection)?;
        // Only a private fixture is corrupted, with the exact publication guard
        // restored before the normal public reader opens the catalog again.
        connection.transaction::<_, diesel::result::Error, _>(|connection| {
            connection.batch_execute("DROP TRIGGER occurrences_require_unpublished_snapshot")?;
            let inserted = sql_query("INSERT INTO asset_occurrences(record_id, occurrence_order, claim_kind) SELECT set_id, COALESCE((SELECT MAX(occurrence_order)+1 FROM asset_occurrences WHERE record_id=set_id), 0), 'software_rom_entry' FROM catalog_sets WHERE list_order=?")
                .bind::<BigInt, _>(title_order)
                .execute(connection)?;
            assert_eq!(inserted, 1, "orphan attack must modify exactly one title");
            connection.batch_execute(&guard.sql)
        })?;
        drop(connection);
        let error = fixture
            .verify_same()
            .err()
            .ok_or("orphan occurrence outside the native hierarchy was accepted")?
            .to_string();
        assert!(
            error.contains("inconsistent owner links"),
            "wrong failure: {error}"
        );
    }
    Ok(())
}

#[test]
fn absent_empty_defaults_raw_invalid_values_and_vendor_gaps_match() -> VerifyResult {
    let xml = "<softwarelists build='v'><vendor/><softwarelist name='list' description=''><notes/><vendor/><software name='g'><publisher/><description/><year/><info name='missing'/><info name='empty' value=''/><part name='p' interface='i'><vendor/><dataarea name='a' size='bad'><rom name='' size='invalid' offset='' crc='NULL' sha1='bad'/><vendor/><rom status='nodump'/></dataarea><diskarea name='d'><disk name='disc' sha1='bad'/></diskarea><dipswitch name='dip' tag='t' mask='m'><dipvalue name='v' value='0'/></dipswitch></part></software></softwarelist></softwarelists>";
    Published::new(xml)?.verify_same()
}

#[test]
fn namespace_and_vendor_attributes_keep_recognized_qname_positions() -> VerifyResult {
    let xml = "<softwarelists xmlns:v='urn:vendor' v:extra='x' build=''><softwarelist v:extra='x' name='list'><software name='g' v:extra='x'><description/><year/><publisher/><part v:extra='x' name='p' interface='i'><dataarea v:extra='x' name='a' size='1'><rom v:extra='x' name='r' crc='11223344' size='1'/></dataarea></part></software></softwarelist></softwarelists>";
    Published::new(xml)?.verify_same()
}

#[test]
fn empty_plural_and_bare_list_do_not_invent_wrapper_facts() -> VerifyResult {
    for xml in ["<softwarelists/>", "<softwarelists build=''/>"] {
        Published::new(xml)?.verify_same()?;
    }
    Published::new(&list(&item("g")))?.verify_same()
}

#[test]
fn gzip_and_both_utf16_byte_orders_preserve_field_coordinates() -> VerifyResult {
    use std::io::Write;

    let xml = format!("<?xml version='1.0' encoding='UTF-16'?>{FULL}");
    for little_endian in [true, false] {
        let mut bytes = if little_endian {
            vec![0xFF, 0xFE]
        } else {
            vec![0xFE, 0xFF]
        };
        for unit in xml.encode_utf16() {
            bytes.extend(if little_endian {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        Published::new_bytes(&bytes)?.verify_same()?;
    }
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(FULL.as_bytes())?;
    Published::new_bytes(&gzip.finish()?)?.verify_same()
}

#[test]
fn title_and_list_pages_exhaust_all_boundaries_and_repeated_names() -> VerifyResult {
    for count in [
        1,
        PAGE_SIZE - 1,
        PAGE_SIZE,
        PAGE_SIZE + 1,
        2 * PAGE_SIZE,
        2 * PAGE_SIZE + 1,
    ] {
        let titles = (0..count).map(|_| item("same")).collect::<String>();
        let fixture = Published::new(&list(&titles))?;
        fixture.verify_same()?;
        assert!(
            fixture
                .verify_xml(&list(&(titles + &item("extra"))))
                .is_err()
        );
        let lists = (0..count).map(|_| list(&item("same"))).collect::<String>();
        Published::new(&format!("<softwarelists>{lists}</softwarelists>"))?.verify_same()?;
    }
    Ok(())
}

#[test]
fn complete_media_batches_include_the_last_payload() -> VerifyResult {
    let count = 2 * super::OCCURRENCE_BATCH_SIZE + 1;
    let roms = (0..count)
        .map(|index| format!("<rom name='r{index}' size='1'/>"))
        .collect::<String>();
    let xml = list(&format!(
        "<software name='g'><description/><year/><publisher/><part name='p' interface='i'><dataarea name='a' size='1024'>{roms}</dataarea></part></software>"
    ));
    let fixture = Published::new(&xml)?;
    fixture.verify_same()?;
    let last = format!("<rom name='r{}' size='1'/>", count - 1);
    let changed = xml.replace(&last, &last.replace("size='1'", "size='2'"));
    let error = fixture
        .verify_xml(&changed)
        .err()
        .ok_or("last ROM ignored")?
        .to_string();
    assert!(error.contains("size_text"), "wrong failure: {error}");
    Ok(())
}

#[test]
fn source_missing_extra_reordered_and_late_invalid_owners_are_rejected() -> VerifyResult {
    let fixture = Published::new(&list(&(item("a") + &item("b"))))?;
    fixture.verify_same()?;
    for xml in [
        list(&item("a")),
        list(&(item("a") + &item("b") + &item("c"))),
        list(&(item("b") + &item("a"))),
        list(&(item("a") + &item("b"))) + "<trailing/>",
    ] {
        assert!(fixture.verify_xml(&xml).is_err(), "accepted {xml}");
    }
    Ok(())
}

#[test]
fn changes_to_each_pinned_scalar_are_rejected_against_healthy_native_data() -> VerifyResult {
    let fixture = Published::new(FULL)?;
    fixture.verify_same()?;
    for (before, after) in [
        ("build=\"\"", "build=\"new\""),
        ("name=\"list\"", "name=\"next\""),
        ("description=\"List\"", "description=\"Other\""),
        ("name=\"game\"", "name=\"other\""),
        ("cloneof=\"parent\"", "cloneof=\"other\""),
        ("supported=\"partial\"", "supported=\"no\""),
        (" Game ", " Next "),
        (">1990<", ">1991<"),
        (">Maker<", ">Other<"),
        ("<notes/>", "<notes>different</notes>"),
        ("Late list notes", "Other list notes"),
        ("name=\"serial\"", "name=\"region\""),
        ("value=\"\"", "value=\"other\""),
        ("name=\"compatibility\"", "name=\"different\""),
        ("value=\"yes\"", "value=\"no\""),
        ("name=\"cart\"", "name=\"different\""),
        ("interface=\"cart\"", "interface=\"other\""),
        ("name=\"slot\"", "name=\"other\""),
        ("value=\"cart\"", "value=\"other\""),
        ("name=\"rom\"", "name=\"other\""),
        ("size=\"0x0010\"", "size=\"0x0020\""),
        ("width=\"16\"", "width=\"32\""),
        ("endianness=\"big\"", "endianness=\"little\""),
        ("name=\"disk\"", "name=\"other\""),
        ("name=\"disk-image\"", "name=\"other-image\""),
        ("name=\"game.bin\"", "name=\"other.bin\""),
        ("size=\"0004\"", "size=\"0005\""),
        ("crc=\"AABBCCDD\"", "crc=\"11223344\""),
        (
            "sha1=\"ABCDEF0123456789ABCDEF0123456789ABCDEF01\"",
            "sha1=\"1111111111111111111111111111111111111111\"",
        ),
        ("offset=\"0x0000\"", "offset=\"0x0001\""),
        ("value=\"00\"", "value=\"01\""),
        ("status=\"baddump\"", "status=\"good\""),
        ("loadflag=\"load16_word_swap\"", "loadflag=\"load16_word\""),
        (
            "sha1=\"0123456789ABCDEF0123456789ABCDEF01234567\"",
            "sha1=\"2222222222222222222222222222222222222222\"",
        ),
        ("status=\"good\"", "status=\"nodump\""),
        ("writeable=\"yes\"", "writeable=\"no\""),
        ("name=\"Region\"", "name=\"Other\""),
        ("tag=\"REGION\"", "tag=\"OTHER\""),
        ("mask=\"0x03\"", "mask=\"0x04\""),
        ("name=\"World\"", "name=\"Other\""),
        ("value=\"1\"", "value=\"2\""),
        ("default=\"yes\"", "default=\"no\""),
    ] {
        assert!(FULL.contains(before), "missing fixture witness: {before}");
        let changed = FULL.replacen(before, after, 1);
        mame_coalesce::mame_softwarelist::SoftwareListCatalog::parse(changed.as_bytes())?;
        assert!(
            fixture.verify_xml(&changed).is_err(),
            "undetected field: {before}"
        );
    }
    Ok(())
}
