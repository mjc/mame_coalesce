-- DESIGN ONLY. Run in a FRESH in-memory database with the complete candidate
-- emitted by assemble.py already applied; no stubs or earlier family fixture.
-- From the active repository devenv, at the repository root:
-- { python3 docs/schema-candidate/assemble.py --emit && cat docs/schema-candidate/software_field_witnesses.sql; } | sqlite3 -bail :memory:
-- software_field_check.py runs this same SQL and validates the TSV against DDL.
-- Constructed native facts prove field storage/position states, not parser EOF,
-- raw enum-token recognition, checked numeric projections or load qualification.
PRAGMA foreign_keys = ON;
BEGIN;
CREATE TEMP TABLE software_field_assertions (
    label TEXT PRIMARY KEY,
    ok INTEGER NOT NULL CHECK (ok = 1)
) STRICT;

INSERT INTO catalog_publishers VALUES (1, 'software-fields', 'Software fields', NULL);
INSERT INTO catalogs VALUES (1, 1, 'software-fields', 'Software fields');
INSERT INTO catalog_source_files VALUES (1, zeroblob(32), NULL, 1000, 'software-field-fixture', 'zstd');
INSERT INTO catalog_reading_rules VALUES
    (1, 'software-fields', 'software', 'plural-compatible', 'mame0289', 'constructed', 'DOC12-38114');
INSERT INTO catalog_coverage VALUES (1, 'unknown');
INSERT INTO catalog_editions
    (edition_id, catalog_id, source_file_id, reading_rules_id, coverage_id)
VALUES (1, 1, 1, 1, 1);
INSERT INTO software_documents
    (edition_id, envelope_kind, location_view, start_line, start_column, end_line, end_column, column_convention)
VALUES (1, 'plural_lists', 'transport_decoded_xml_text', 1, 1, 100, 1, 'one_based_unicode_scalar');
INSERT INTO software_wrapper_headers
    (wrapper_id, edition_id, build, location_view, start_line, start_column, end_line, end_column, column_convention)
VALUES (10, 1, '', 'transport_decoded_xml_text', 2, 1, 90, 1, 'one_based_unicode_scalar');
INSERT INTO catalog_set_groups VALUES (20, 1, 'software_list'), (21, 1, 'software_list');
INSERT INTO software_lists
    (set_group_id, name, description, location_view, start_line, start_column, end_line, end_column, column_convention)
VALUES (20, '', '', 'transport_decoded_xml_text', 3, 1, 50, 1, 'one_based_unicode_scalar'),
       (21, 'omitted-description', NULL, 'transport_decoded_xml_text', 51, 1, 80, 1, 'one_based_unicode_scalar');

INSERT INTO catalog_source_elements VALUES
    (11,1,'software_list_wrapper_entry'), (12,1,'software_list_wrapper_entry'),
    (30,1,'software_list_notes'),
    (100,1,'software_item'), (101,1,'software_item'),
    (110,1,'software_title_text_element'), (111,1,'software_title_text_element'),
    (112,1,'software_title_text_element'), (113,1,'software_title_text_element'),
    (114,1,'software_title_text_element'), (115,1,'software_title_text_element'),
    (116,1,'software_title_text_element'),
    (120,1,'software_title_info'), (121,1,'software_title_info'),
    (122,1,'software_shared_feature'), (123,1,'software_shared_feature'),
    (200,1,'software_part'),
    (210,1,'software_part_feature'), (211,1,'software_part_feature'),
    (220,1,'software_part_switch'),
    (230,1,'software_part_switch_value'), (231,1,'software_part_switch_value'),
    (300,1,'software_area'), (301,1,'software_area'), (302,1,'software_area'),
    (400,1,'software_rom_entry'), (401,1,'software_rom_entry'),
    (402,1,'software_rom_entry'), (403,1,'software_rom_entry'),
    (410,1,'software_disk_entry'), (411,1,'software_disk_entry'),
    (412,1,'software_disk_entry'), (413,1,'software_disk_entry');
INSERT INTO software_list_wrapper_entries VALUES (11,20,10,0), (12,21,10,1);
INSERT INTO software_list_notes VALUES (30,20,'',0,4,1);
INSERT INTO catalog_sets VALUES (100,20,'',1,5,1), (101,21,'omitted-defaults',0,52,1);
INSERT INTO software_titles VALUES (100,'yes',1), (101,'yes',0);
INSERT INTO software_title_text_elements VALUES
    (110,100,0,'',0,6,1), (111,100,1,'',1,7,1),
    (112,100,2,'',2,8,1), (113,100,3,'',3,9,1),
    (114,101,0,'',0,53,1), (115,101,1,'',1,54,1), (116,101,2,'',2,55,1);
INSERT INTO software_title_info VALUES (120,100,'','',4,10,1), (121,100,'',NULL,5,11,1);
INSERT INTO software_shared_features VALUES (122,100,'','',6,12,1), (123,100,'',NULL,7,13,1);
INSERT INTO software_parts VALUES (200,100,'','',8,14,1);
INSERT INTO software_part_features VALUES (210,200,'','',0,15,1), (211,200,'',NULL,1,16,1);
INSERT INTO software_part_switches VALUES (220,200,'','','',2,17,1);
INSERT INTO software_part_switch_values VALUES
    (230,220,'','',0,1,0,18,1), (231,220,'','',0,0,1,19,1);
INSERT INTO software_areas VALUES
    (300,200,'data',3,20,1), (301,200,'data',4,30,1), (302,200,'disk',5,35,1);
INSERT INTO software_data_areas VALUES
    (300,'','',8,1,'little',1), (301,'omitted-defaults','+010',8,0,'little',0);
INSERT INTO software_disk_areas VALUES (302,'');
INSERT INTO catalog_media_entries(media_entry_id) VALUES
    (400),(401),(402),(403),(410),(411),(412),(413);
INSERT INTO software_rom_load_entries
    (media_entry_id,data_area_id,name,size_text,offset_text,value,loadflag,status,status_specified,source_order,source_line,source_column)
VALUES (400,300,'','','+010','','load16_byte','good',1,0,21,1),
       (401,301,NULL,NULL,NULL,NULL,NULL,'good',0,0,31,1),
       (402,300,'numeric-controls','010','0Xf','not-a-number',NULL,'good',0,1,22,1),
       (403,300,NULL,'overflow-or-junk','','0x+2','fill','nodump',1,2,23,1);
INSERT INTO software_disks
    (media_entry_id,disk_area_id,name,status,status_specified,writeable,writeable_specified,source_order,source_line,source_column)
VALUES (410,302,'','good',1,0,1,0,36,1),
       (411,302,'omitted-defaults','good',0,0,0,1,37,1),
       (412,302,'invalid-hash','baddump',1,1,1,2,38,1),
       (413,302,'valid-hash','good',0,0,0,3,39,1);
-- Raw malformed numbers/empty names still declare source requirements. These
-- relations make no claim about executable recipes or whole-file eligibility.
INSERT INTO software_required_files VALUES (500,400),(501,401),(502,402);
INSERT INTO software_file_load_steps VALUES (400,500),(401,501),(402,502),(403,NULL);
INSERT INTO catalog_relationships(relationship_id,assertion_key,origin,edition_id)
VALUES (900,'software-field-empty-clone','source',1);
INSERT INTO reported_catalog_relationships VALUES (900,'software_cloneof');
INSERT INTO software_clone_links VALUES (100,'',900);
INSERT INTO hash_values VALUES (700,'crc32',x'1234abcd'), (701,'sha1',zeroblob(20));
INSERT INTO catalog_entry_hashes VALUES
    (600,400,'crc',0,'empty','unknown',NULL,NULL),
    (601,400,'sha1',0,'invalid','unknown',NULL,'bad'),
    (602,402,'crc',0,'value','unknown',700,'1234ABCD'),
    (603,402,'sha1',0,'value','unknown',701,NULL),
    (604,403,'crc',0,'invalid','unknown',NULL,'xyz'),
    (605,403,'sha1',0,'empty','unknown',NULL,NULL),
    (606,410,'sha1',0,'empty','unknown',NULL,NULL),
    (607,412,'sha1',0,'invalid','unknown',NULL,'bad'),
    (608,413,'sha1',0,'value','chd_header_sha1',701,NULL);
INSERT INTO invalid_catalog_entry_hashes VALUES
    (601,'invalid_hash'),(604,'invalid_hash'),(607,'invalid_hash');

-- Every accepted QName code is instantiated against a real typed owner.
-- Attribute order is local and independent of all element-child ordinals.
INSERT INTO software_wrapper_attribute_positions(wrapper_id,field_kind,source_order,source_line,source_column)
VALUES (10,0,0,2,16);
INSERT INTO software_list_attribute_positions(set_group_id,field_kind,source_order,source_line,source_column)
VALUES (20,0,0,3,15),(20,1,1,3,23),(21,0,0,51,15);
INSERT INTO software_title_attribute_positions(set_id,field_kind,source_order,source_line,source_column,relationship_id)
VALUES (100,0,0,5,11,NULL),(100,1,1,5,19,900),(100,2,2,5,30,NULL),(101,0,0,52,11,NULL);
INSERT INTO software_title_info_attribute_positions(source_element_id,field_kind,source_order,source_line,source_column)
VALUES (120,0,0,10,7),(120,1,1,10,15),(121,0,0,11,7);
INSERT INTO software_shared_feature_attribute_positions(source_element_id,field_kind,source_order,source_line,source_column)
VALUES (122,0,0,12,13),(122,1,1,12,21),(123,0,0,13,13);
INSERT INTO software_part_attribute_positions(part_id,field_kind,source_order,source_line,source_column)
VALUES (200,0,0,14,7),(200,1,1,14,15);
INSERT INTO software_part_feature_attribute_positions(source_element_id,field_kind,source_order,source_line,source_column)
VALUES (210,0,0,15,10),(210,1,1,15,18),(211,0,0,16,10);
INSERT INTO software_part_switch_attribute_positions(switch_id,field_kind,source_order,source_line,source_column)
VALUES (220,0,0,17,12),(220,1,1,17,20),(220,2,2,17,27);
INSERT INTO software_part_switch_value_attribute_positions(source_element_id,field_kind,source_order,source_line,source_column)
VALUES (230,0,0,18,11),(230,1,1,18,19),(230,2,2,18,28),(231,0,0,19,11),(231,1,1,19,19);
INSERT INTO software_data_area_attribute_positions(area_id,field_kind,source_order,source_line,source_column)
VALUES (300,0,0,20,11),(300,1,1,20,19),(300,2,2,20,27),(300,3,3,20,37),
       (301,0,0,30,11),(301,1,1,30,37);
INSERT INTO software_disk_area_attribute_positions(area_id,field_kind,source_order,source_line,source_column)
VALUES (302,0,0,35,11);
INSERT INTO software_rom_attribute_positions(media_entry_id,field_kind,source_order,source_line,source_column,reported_hash_id)
VALUES (400,0,0,21,6,NULL),(400,1,1,21,14,NULL),(400,2,2,21,22,600),(400,3,3,21,29,601),
       (400,4,4,21,40,NULL),(400,5,5,21,54,NULL),(400,6,6,21,63,NULL),(400,7,7,21,77,NULL),
       (402,0,0,22,6,NULL),(402,1,1,22,30,NULL),(402,2,2,22,41,602),(402,3,3,22,56,603),
       (402,4,4,22,104,NULL),(402,5,5,22,117,NULL),
       (403,1,0,23,6,NULL),(403,2,1,23,30,604),(403,3,2,23,40,605),
       (403,4,3,23,48,NULL),(403,5,4,23,58,NULL),(403,6,5,23,73,NULL),(403,7,6,23,89,NULL);
INSERT INTO software_disk_attribute_positions(media_entry_id,field_kind,source_order,source_line,source_column,reported_hash_id)
VALUES (410,0,0,36,7,NULL),(410,1,1,36,15,606),(410,2,2,36,23,NULL),(410,3,3,36,37,NULL),
       (411,0,0,37,7,NULL),
       (412,0,0,38,7,NULL),(412,1,1,38,28,607),(412,2,2,38,39,NULL),(412,3,3,38,56,NULL),
       (413,0,0,39,7,NULL),(413,1,1,39,26,608);

-- These assertions observe native values AND positions, not ledger row counts.
INSERT INTO software_field_assertions VALUES
    ('wrapper-present-empty',(SELECT build='' AND EXISTS(SELECT 1 FROM software_wrapper_attribute_positions WHERE wrapper_id=10 AND field_kind=0) FROM software_wrapper_headers WHERE wrapper_id=10)),
    ('list-required-empty-and-optional-empty',(SELECT name='' AND description='' AND (SELECT count(*) FROM software_list_attribute_positions WHERE set_group_id=20)=2 FROM software_lists WHERE set_group_id=20)),
    ('list-description-absent',(SELECT description IS NULL AND NOT EXISTS(SELECT 1 FROM software_list_attribute_positions WHERE set_group_id=21 AND field_kind=1) FROM software_lists WHERE set_group_id=21)),
    ('title-name-empty',(SELECT set_name='' FROM catalog_sets WHERE set_id=100)),
    ('clone-empty-is-declaration',(SELECT target_name='' AND relationship_id=900 AND EXISTS(SELECT 1 FROM software_title_attribute_positions WHERE set_id=100 AND field_kind=1 AND relationship_id=900) FROM software_clone_links WHERE set_id=100)),
    ('clone-absent',NOT EXISTS(SELECT 1 FROM software_clone_links WHERE set_id=101) AND NOT EXISTS(SELECT 1 FROM software_title_attribute_positions WHERE set_id=101 AND field_kind=1)),
    ('supported-explicit-default',(SELECT supported='yes' AND supported_specified=1 AND EXISTS(SELECT 1 FROM software_title_attribute_positions WHERE set_id=100 AND field_kind=2) FROM software_titles WHERE set_id=100)),
    ('supported-omitted-default',(SELECT supported='yes' AND supported_specified=0 AND NOT EXISTS(SELECT 1 FROM software_title_attribute_positions WHERE set_id=101 AND field_kind=2) FROM software_titles WHERE set_id=101)),
    ('list-notes-empty',(SELECT text_value='' FROM software_list_notes WHERE set_group_id=20)),
    ('list-notes-absent',NOT EXISTS(SELECT 1 FROM software_list_notes WHERE set_group_id=21)),
    ('title-all-text-kinds-empty',(SELECT count(*)=4 AND min(text_value='') AND min(field_kind)=0 AND max(field_kind)=3 FROM software_title_text_elements WHERE set_id=100)),
    ('title-optional-notes-absent',(SELECT count(*)=3 AND min(text_value='') AND max(field_kind)=2 FROM software_title_text_elements WHERE set_id=101)),
    ('info-present-empty',(SELECT name='' AND value='' AND EXISTS(SELECT 1 FROM software_title_info_attribute_positions WHERE source_element_id=120 AND field_kind=1) FROM software_title_info WHERE source_element_id=120)),
    ('info-value-absent',(SELECT name='' AND value IS NULL AND NOT EXISTS(SELECT 1 FROM software_title_info_attribute_positions WHERE source_element_id=121 AND field_kind=1) FROM software_title_info WHERE source_element_id=121)),
    ('sharedfeat-present-empty',(SELECT name='' AND value='' AND EXISTS(SELECT 1 FROM software_shared_feature_attribute_positions WHERE source_element_id=122 AND field_kind=1) FROM software_shared_features WHERE source_element_id=122)),
    ('sharedfeat-value-absent',(SELECT name='' AND value IS NULL AND NOT EXISTS(SELECT 1 FROM software_shared_feature_attribute_positions WHERE source_element_id=123 AND field_kind=1) FROM software_shared_features WHERE source_element_id=123)),
    ('part-required-empty',(SELECT name='' AND interface='' FROM software_parts WHERE part_id=200)),
    ('feature-present-empty',(SELECT name='' AND value='' AND EXISTS(SELECT 1 FROM software_part_feature_attribute_positions WHERE source_element_id=210 AND field_kind=1) FROM software_part_features WHERE source_element_id=210)),
    ('feature-value-absent',(SELECT name='' AND value IS NULL AND NOT EXISTS(SELECT 1 FROM software_part_feature_attribute_positions WHERE source_element_id=211 AND field_kind=1) FROM software_part_features WHERE source_element_id=211)),
    ('switch-required-raw-text',(SELECT name='' AND tag='' AND mask='' FROM software_part_switches WHERE switch_id=220)),
    ('dipvalue-explicit-default',(SELECT name='' AND value='' AND is_default=0 AND default_specified=1 AND EXISTS(SELECT 1 FROM software_part_switch_value_attribute_positions WHERE source_element_id=230 AND field_kind=2) FROM software_part_switch_values WHERE source_element_id=230)),
    ('dipvalue-omitted-default',(SELECT is_default=0 AND default_specified=0 AND NOT EXISTS(SELECT 1 FROM software_part_switch_value_attribute_positions WHERE source_element_id=231 AND field_kind=2) FROM software_part_switch_values WHERE source_element_id=231)),
    ('dataarea-explicit-defaults',(SELECT name='' AND declared_size_text='' AND width=8 AND width_specified=1 AND endianness='little' AND endianness_specified=1 AND (SELECT count(*) FROM software_data_area_attribute_positions WHERE area_id=300 AND field_kind IN(2,3))=2 FROM software_data_areas WHERE area_id=300)),
    ('dataarea-omitted-defaults-raw-invalid',(SELECT declared_size_text='+010' AND width=8 AND width_specified=0 AND endianness='little' AND endianness_specified=0 AND NOT EXISTS(SELECT 1 FROM software_data_area_attribute_positions WHERE area_id=301 AND field_kind IN(2,3)) FROM software_data_areas WHERE area_id=301)),
    ('diskarea-required-empty',(SELECT name='' FROM software_disk_areas WHERE area_id=302)),
    ('rom-empty-raw-invalid-and-explicit-default',(SELECT name='' AND size_text='' AND offset_text='+010' AND value='' AND loadflag='load16_byte' AND status='good' AND status_specified=1 AND EXISTS(SELECT 1 FROM software_rom_attribute_positions WHERE media_entry_id=400 AND field_kind=6) FROM software_rom_load_entries WHERE media_entry_id=400)),
    ('rom-all-optional-absent',(SELECT name IS NULL AND size_text IS NULL AND offset_text IS NULL AND value IS NULL AND loadflag IS NULL AND status='good' AND status_specified=0 AND NOT EXISTS(SELECT 1 FROM software_rom_attribute_positions WHERE media_entry_id=401) FROM software_rom_load_entries WHERE media_entry_id=401)),
    ('rom-valid-numeric-lexemes-unchanged',(SELECT size_text='010' AND offset_text='0Xf' AND value='not-a-number' FROM software_rom_load_entries WHERE media_entry_id=402)),
    ('rom-fill-raw-invalid-unchanged',(SELECT size_text='overflow-or-junk' AND offset_text='' AND value='0x+2' AND loadflag='fill' FROM software_rom_load_entries WHERE media_entry_id=403)),
    ('disk-explicit-defaults',(SELECT name='' AND status='good' AND status_specified=1 AND writeable=0 AND writeable_specified=1 AND (SELECT count(*) FROM software_disk_attribute_positions WHERE media_entry_id=410 AND field_kind IN(2,3))=2 FROM software_disks WHERE media_entry_id=410)),
    ('disk-omitted-defaults',(SELECT status='good' AND status_specified=0 AND writeable=0 AND writeable_specified=0 AND NOT EXISTS(SELECT 1 FROM software_disk_attribute_positions WHERE media_entry_id=411 AND field_kind IN(2,3)) FROM software_disks WHERE media_entry_id=411)),
    ('disk-explicit-nondefaults',(SELECT status='baddump' AND status_specified=1 AND writeable=1 AND writeable_specified=1 FROM software_disks WHERE media_entry_id=412));

-- All three digest slots exercise absent / empty / invalid / normalized value.
-- Hash assertions stay entry-owned even on fill; unknown scope grants no UUID.
INSERT INTO software_field_assertions VALUES
    ('rom-hashes-absent',NOT EXISTS(SELECT 1 FROM catalog_entry_hashes WHERE media_entry_id=401)),
    ('disk-hash-absent',NOT EXISTS(SELECT 1 FROM catalog_entry_hashes WHERE media_entry_id=411)),
    ('rom-crc-empty',(SELECT presence='empty' AND declared_text='' AND hash_id IS NULL FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE reported_hash_id=600)),
    ('rom-crc-invalid',(SELECT presence='invalid' AND declared_text='xyz' AND hash_id IS NULL FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE reported_hash_id=604)),
    ('rom-crc-value-spelling',(SELECT presence='value' AND declared_text='1234ABCD' AND hash_id=700 FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE reported_hash_id=602)),
    ('rom-sha1-empty',(SELECT presence='empty' AND declared_text='' AND hash_id IS NULL FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE reported_hash_id=605)),
    ('rom-sha1-invalid',(SELECT presence='invalid' AND declared_text='bad' AND hash_id IS NULL FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE reported_hash_id=601)),
    ('rom-sha1-normalized',(SELECT presence='value' AND declared_text=lower(hex(zeroblob(20))) AND hash_id=701 FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE reported_hash_id=603)),
    ('disk-sha1-empty',(SELECT presence='empty' AND declared_text='' FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE reported_hash_id=606)),
    ('disk-sha1-invalid',(SELECT presence='invalid' AND declared_text='bad' FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE reported_hash_id=607)),
    ('disk-sha1-normalized',(SELECT presence='value' AND hash_scope='chd_header_sha1' AND declared_text=lower(hex(zeroblob(20))) FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE reported_hash_id=608));

-- Wrapper omission uses the same valid physical root, with its position removed.
SAVEPOINT omitted_wrapper_build;
DELETE FROM software_wrapper_attribute_positions WHERE wrapper_id=10;
UPDATE software_wrapper_headers SET build=NULL WHERE wrapper_id=10;
INSERT INTO software_field_assertions VALUES
    ('wrapper-build-absent',(SELECT build IS NULL AND NOT EXISTS(SELECT 1 FROM software_wrapper_attribute_positions WHERE wrapper_id=10) FROM software_wrapper_headers WHERE wrapper_id=10));
ROLLBACK TO omitted_wrapper_build;
RELEASE omitted_wrapper_build;

-- Missing required text is a cross-table audit failure, not a NOT NULL failure.
SAVEPOINT missing_required_text;
DELETE FROM software_title_text_elements WHERE source_element_id=110;
INSERT INTO software_field_assertions VALUES
    ('required-description-audited',EXISTS(SELECT 1 FROM candidate_software_cardinality_problems WHERE problem='software_title_required_text_missing' AND owner_id=100));
ROLLBACK TO missing_required_text;
RELEASE missing_required_text;

INSERT INTO software_field_assertions VALUES
    ('foreign-keys-clean',(SELECT count(*)=0 FROM pragma_foreign_key_check));
COMMIT;
SELECT 'software field witnesses passed' AS result;
