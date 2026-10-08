-- Run from the repository root with:
-- sqlite3 :memory: < docs/schema-candidate/mame_witnesses.sql
-- Bounded design witness; common tables below are only the required keys.
.bail on
PRAGMA foreign_keys = ON;

CREATE TABLE catalog_reading_rules (
    reading_rules_id INTEGER PRIMARY KEY,
    rules_key TEXT NOT NULL UNIQUE,
    format_family TEXT NOT NULL,
    dialect TEXT NOT NULL,
    specification_version TEXT NOT NULL,
    parser_version TEXT NOT NULL,
    rules_version TEXT NOT NULL
) STRICT;
CREATE TABLE catalog_editions (
    edition_id INTEGER PRIMARY KEY,
    reading_rules_id INTEGER NOT NULL REFERENCES catalog_reading_rules(reading_rules_id)
) STRICT;
CREATE TABLE catalog_source_elements (
    source_element_id INTEGER PRIMARY KEY,
    edition_id INTEGER NOT NULL REFERENCES catalog_editions(edition_id),
    element_kind TEXT NOT NULL,
    UNIQUE(source_element_id,edition_id)
) STRICT;
CREATE TABLE catalog_set_groups (
    set_group_id INTEGER PRIMARY KEY,
    edition_id INTEGER NOT NULL REFERENCES catalog_editions(edition_id),
    group_kind TEXT NOT NULL,
    UNIQUE(set_group_id,edition_id)
) STRICT;
CREATE TABLE catalog_sets (
    set_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_group_id INTEGER NOT NULL REFERENCES catalog_set_groups(set_group_id),
    set_name TEXT NOT NULL,
    source_order INTEGER NOT NULL,
    UNIQUE(set_group_id,source_order)
) STRICT;
CREATE TABLE catalog_media_entries (
    media_entry_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id)
) STRICT;
CREATE TABLE reported_catalog_relationships (relationship_id INTEGER PRIMARY KEY) STRICT;
CREATE TABLE hash_values (hash_id INTEGER PRIMARY KEY, algorithm TEXT NOT NULL, bytes BLOB NOT NULL) STRICT;
CREATE TABLE catalog_entry_hashes (
    reported_hash_id INTEGER PRIMARY KEY,
    media_entry_id INTEGER NOT NULL REFERENCES catalog_media_entries(media_entry_id),
    source_hash_field TEXT NOT NULL,
    field_occurrence INTEGER NOT NULL,
    presence TEXT NOT NULL,
    hash_scope TEXT NOT NULL,
    hash_id INTEGER REFERENCES hash_values(hash_id),
    reported_text TEXT,
    UNIQUE(media_entry_id,source_hash_field,field_occurrence)
) STRICT;

.read docs/schema-candidate/mame.sql

CREATE TEMP TABLE witness_assert (ok INTEGER NOT NULL CHECK(ok=1)) STRICT;
INSERT INTO catalog_reading_rules VALUES
    (1,'mame-observed-v3','mame','mame-listxml','0.289','fixture-parser','mame-observed-compat-declared-text-v3');
INSERT INTO catalog_editions VALUES (1,1);
INSERT INTO catalog_set_groups VALUES (10,1,'root');
INSERT INTO catalog_source_elements VALUES
    (100,1,'mame_machine'), (101,1,'mame_machine_text'),
    (102,1,'mame_rom'), (103,1,'mame_disk'), (104,1,'mame_sample'),
    (105,1,'mame_device'), (106,1,'mame_device_extension'),
    (107,1,'mame_device_instance'), (108,1,'mame_device_extension');
INSERT INTO catalog_sets VALUES (100,10,'fixture-machine',0);
INSERT INTO catalog_media_entries VALUES (102),(103),(104);
INSERT INTO mame_documents (edition_id,build,debug,debug_specified,mameconfig,source_line,source_column,
    extent_view,extent_start,extent_end,location_view,start_line,start_column,end_line,end_column,column_convention)
VALUES (1,'0.289',0,0,'10',1,1,'transport_decoded_xml_bytes',0,120,
    'transport_decoded_xml_text',1,1,12,1,'one_based_unicode_scalar');
INSERT INTO catalog_editions VALUES (2,1);
INSERT OR IGNORE INTO mame_documents (edition_id,build,debug,debug_specified,mameconfig,source_line,source_column)
VALUES (2,'0.289',0,0,'10',1,1);
INSERT INTO witness_assert SELECT changes()=0;
INSERT INTO mame_machines (set_id,sourcefile,isbios,isbios_specified,isdevice,isdevice_specified,
    ismechanical,ismechanical_specified,runnable,runnable_specified)
VALUES (100,'fixture.cpp',0,0,0,0,0,0,1,0);
INSERT INTO mame_machine_text_elements VALUES (101,100,'description','Fixture Machine',0,2,3);
INSERT INTO mame_roms VALUES (102,100,'', '18446744073709551615',NULL,NULL,'0xffffffffffffffff',
    'good',0,0,0,1,3,3);
INSERT INTO mame_disks VALUES (103,100,'fixture.chd',NULL,'01',0,0,'good',0,0,0,2,3,3);
INSERT INTO mame_samples VALUES (104,100,'',3,4,3);
INSERT INTO mame_devices VALUES (105,100,'floppy',NULL,NULL,NULL,'floppy',4,5,4);
INSERT INTO mame_device_extensions VALUES (106,105,'dsk',0,5,20);
INSERT INTO mame_device_instances VALUES (107,105,'drive0','FDD',1,5,31);
INSERT INTO mame_device_extensions VALUES (108,105,'img',2,5,44);

-- Each reported assertion has one literal owner and one canonical position.
INSERT INTO reported_catalog_relationships VALUES (1),(2),(3),(4),(5),(6);
INSERT INTO mame_machine_links VALUES
    (100,'cloneof','parent-machine',1),
    (100,'romof','rom-parent',2),
    (100,'sampleof','sample-parent',3);
INSERT INTO mame_machines_attribute_positions
    (set_id,field_kind,relationship_id,source_order,source_line,source_column)
VALUES (100,'name',NULL,0,2,10), (100,'sourcefile',NULL,1,2,30),
    (100,'cloneof',1,2,2,55), (100,'romof',2,3,2,80),
    (100,'sampleof',3,4,2,100);
INSERT INTO catalog_source_elements VALUES (110,1,'mame_device_reference');
INSERT INTO mame_device_references VALUES (110,100,':fixture','device-parent',4,5,6,3);
INSERT INTO mame_device_references_attribute_positions
    (source_element_id,field_kind,relationship_id,source_order,source_line,source_column)
VALUES (110,'tag',NULL,0,6,15), (110,'name',4,1,6,30);
INSERT INTO mame_rom_merges VALUES (102,5,'parent.rom');
INSERT INTO mame_disk_merges VALUES (103,6,'parent.chd');
INSERT INTO mame_rom_claims_attribute_positions
    (media_entry_id,field_kind,relationship_id,source_order,source_line,source_column)
VALUES (102,'merge',5,2,3,75);
INSERT INTO mame_disk_claims_attribute_positions
    (media_entry_id,field_kind,relationship_id,source_order,source_line,source_column)
VALUES (103,'name',NULL,0,4,10), (103,'merge',6,1,4,35);

INSERT INTO hash_values VALUES (1,'crc32',x'deadbeef');
INSERT INTO catalog_entry_hashes VALUES (1,102,'crc',0,'value','whole_file',1,NULL);
INSERT INTO mame_rom_claims_attribute_positions
    (media_entry_id,field_kind,field_occurrence,reported_hash_id,source_order,source_line,source_column)
VALUES (102,'crc',0,1,0,3,58);
INSERT INTO mame_rom_claims_attribute_positions
    (media_entry_id,field_kind,source_order,source_line,source_column)
VALUES (102,'name',1,3,12);

INSERT INTO witness_assert
SELECT size_text='18446744073709551615' AND offset='0xffffffffffffffff'
FROM mame_roms WHERE media_entry_id=102;
INSERT INTO witness_assert
SELECT count(*)=3 AND min(source_order)=0 AND max(source_order)=2
FROM (
    SELECT source_order FROM mame_device_extensions WHERE device_id=105
    UNION ALL SELECT source_order FROM mame_device_instances WHERE device_id=105
);
-- A second observed-v3 instance is retained. Changing only this fixture's
-- reading identity to strict DTD makes the candidate integrity view report it.
INSERT INTO catalog_source_elements VALUES (109,1,'mame_device_instance');
INSERT INTO mame_device_instances VALUES (109,105,'drive1','FDD',3,5,51);
INSERT INTO witness_assert SELECT count(*)=0 FROM candidate_mame_integrity_problems;
UPDATE catalog_reading_rules
SET dialect='strict-dtd',rules_version='mame-strict-dtd-fixture'
WHERE reading_rules_id=1;
INSERT INTO witness_assert
SELECT count(*)=1 AND min(problem)='strict_device_instance_cardinality'
FROM candidate_mame_integrity_problems WHERE owner_id=105 AND edition_id=1;
UPDATE catalog_reading_rules
SET dialect='mame-listxml',rules_version='mame-observed-compat-declared-text-v3'
WHERE reading_rules_id=1;
INSERT INTO witness_assert SELECT count(*)=0 FROM candidate_mame_integrity_problems;
INSERT INTO witness_assert
SELECT position.reported_hash_id=hash.reported_hash_id
   AND position.field_kind='crc' AND position.field_occurrence=0
FROM mame_rom_claims_attribute_positions AS position
JOIN catalog_entry_hashes AS hash USING(reported_hash_id)
WHERE position.media_entry_id=102;
INSERT INTO witness_assert
SELECT count(*)=0 FROM mame_machine_compatibility;

-- Closed enums, field occurrence, hash-position ownership triggers,
-- integer storage classes, and QName/source-order singleton keys are present.
INSERT OR IGNORE INTO mame_features VALUES (900,100,'invented',NULL,NULL,5,1,1);
INSERT INTO witness_assert SELECT changes()=0;
INSERT OR IGNORE INTO mame_rom_claims_attribute_positions
    (media_entry_id,field_kind,field_occurrence,reported_hash_id,source_order,source_line,source_column)
VALUES (102,'crc',0,1,2,3,58);
INSERT INTO witness_assert SELECT changes()=0;
INSERT INTO witness_assert
SELECT count(*)=6 FROM sqlite_schema WHERE type='trigger' AND name LIKE 'mame_%hash_position_owner_%';
INSERT INTO witness_assert
SELECT typeof(source_order)='integer' AND typeof(source_line)='integer' AND typeof(source_column)='integer'
FROM mame_rom_claims_attribute_positions WHERE media_entry_id=102 AND field_kind='name';

-- All six relationship kinds bind the actual literal owner's source field.
INSERT INTO witness_assert
SELECT count(*)=6 FROM (
    SELECT position.relationship_id
    FROM mame_machines_attribute_positions AS position
    JOIN mame_machine_links AS link
      ON link.machine_id=position.set_id AND link.link_kind=position.field_kind
     AND link.relationship_id=position.relationship_id
    UNION ALL
    SELECT position.relationship_id
    FROM mame_device_references_attribute_positions AS position
    JOIN mame_device_references AS link USING(source_element_id,relationship_id)
    WHERE position.field_kind='name'
    UNION ALL
    SELECT position.relationship_id
    FROM mame_rom_claims_attribute_positions AS position
    JOIN mame_rom_merges AS link USING(media_entry_id,relationship_id)
    WHERE position.field_kind='merge'
    UNION ALL
    SELECT position.relationship_id
    FROM mame_disk_claims_attribute_positions AS position
    JOIN mame_disk_merges AS link USING(media_entry_id,relationship_id)
    WHERE position.field_kind='merge'
);

-- Starting from valid rows isolates each required/forbidden relationship FK
-- check: parent, code, occurrence, ordinal, coordinates and all other facts
-- remain valid. The six relationship-bearing fields cannot lose their FK.
UPDATE OR IGNORE mame_machines_attribute_positions SET relationship_id=NULL
WHERE set_id=100 AND field_kind IN ('cloneof','romof','sampleof');
INSERT INTO witness_assert SELECT changes()=0;
UPDATE OR IGNORE mame_device_references_attribute_positions SET relationship_id=NULL
WHERE source_element_id=110 AND field_kind='name';
INSERT INTO witness_assert SELECT changes()=0;
UPDATE OR IGNORE mame_rom_claims_attribute_positions SET relationship_id=NULL
WHERE media_entry_id=102 AND field_kind='merge';
INSERT INTO witness_assert SELECT changes()=0;
UPDATE OR IGNORE mame_disk_claims_attribute_positions SET relationship_id=NULL
WHERE media_entry_id=103 AND field_kind='merge';
INSERT INTO witness_assert SELECT changes()=0;

-- These IDs exist and are unused in the receiving position table; rejection
-- therefore comes from the field-conditioned CHECK rather than FK/UNIQUE.
UPDATE OR IGNORE mame_machines_attribute_positions SET relationship_id=4
WHERE set_id=100 AND field_kind='name';
INSERT INTO witness_assert SELECT changes()=0;
UPDATE OR IGNORE mame_device_references_attribute_positions SET relationship_id=1
WHERE source_element_id=110 AND field_kind='tag';
INSERT INTO witness_assert SELECT changes()=0;
UPDATE OR IGNORE mame_rom_claims_attribute_positions SET relationship_id=3
WHERE media_entry_id=102 AND field_kind='name';
INSERT INTO witness_assert SELECT changes()=0;
UPDATE OR IGNORE mame_disk_claims_attribute_positions SET relationship_id=2
WHERE media_entry_id=103 AND field_kind='name';
INSERT INTO witness_assert SELECT changes()=0;

UPDATE OR IGNORE mame_machines_attribute_positions SET relationship_id=1
WHERE set_id=100 AND field_kind='romof';
INSERT INTO witness_assert SELECT changes()=0;

-- Defer only this negative probe so SQL can inspect the isolated missing
-- reported-subtype FK violation, then roll the probe back before completion.
SAVEPOINT missing_reported_relationship;
PRAGMA defer_foreign_keys=ON;
UPDATE mame_machines_attribute_positions SET relationship_id=999999
WHERE set_id=100 AND field_kind='cloneof';
INSERT INTO witness_assert
SELECT count(*)=1 FROM pragma_foreign_key_check
WHERE "table"='mame_machines_attribute_positions'
  AND parent='reported_catalog_relationships';
ROLLBACK TO missing_reported_relationship;
RELEASE missing_reported_relationship;
PRAGMA defer_foreign_keys=OFF;
INSERT INTO witness_assert SELECT count(*)=0 FROM pragma_foreign_key_check;

-- The populated actual-parent lookup remains index-backed.
.eqp on
SELECT media_entry_id FROM mame_roms WHERE machine_id=100 ORDER BY source_order;
.eqp off
SELECT 'MAME bounded witnesses passed' AS result;
