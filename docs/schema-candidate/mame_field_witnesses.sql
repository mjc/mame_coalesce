-- Independent constructed MAME field witness at signed main e591363.
-- Prerequisites: SQLite CLI >=3.37, repository-root cwd, fresh empty in-memory DB.
-- Run in the repository's active devenv:
--   sqlite3 -bail :memory: < docs/schema-candidate/mame_field_witnesses.sql
-- Or activate explicitly:
--   devenv shell -- sqlite3 -bail :memory: < docs/schema-candidate/mame_field_witnesses.sql
-- Minimal common-key interfaces isolate native field execution. The shared hash
-- state contract is represented explicitly; assembled publication guards, source
-- parser execution/count seals, actual source spans and UUID linking are outside
-- this fixture. The TEMP presence audit below is a TEST detecting layer only.
.bail on
PRAGMA foreign_keys=ON;
CREATE TABLE catalog_reading_rules (
 reading_rules_id INTEGER PRIMARY KEY, rules_key TEXT NOT NULL UNIQUE,
 format_family TEXT NOT NULL, dialect TEXT NOT NULL,
 specification_version TEXT NOT NULL, parser_version TEXT NOT NULL, rules_version TEXT NOT NULL
) STRICT;
CREATE TABLE catalog_editions (
 edition_id INTEGER PRIMARY KEY,
 reading_rules_id INTEGER NOT NULL REFERENCES catalog_reading_rules(reading_rules_id)
) STRICT;
CREATE TABLE catalog_source_elements (
 source_element_id INTEGER PRIMARY KEY,
 edition_id INTEGER NOT NULL REFERENCES catalog_editions(edition_id),
 element_kind TEXT NOT NULL, UNIQUE(source_element_id,edition_id)
) STRICT;
CREATE TABLE catalog_set_groups (
 set_group_id INTEGER PRIMARY KEY,
 edition_id INTEGER NOT NULL REFERENCES catalog_editions(edition_id), group_kind TEXT NOT NULL
) STRICT;
CREATE TABLE catalog_sets (
 set_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
 set_group_id INTEGER NOT NULL REFERENCES catalog_set_groups(set_group_id),
 set_name TEXT NOT NULL, source_order INTEGER NOT NULL CHECK(source_order>=0),
 source_line INTEGER NOT NULL CHECK(source_line>0), source_column INTEGER NOT NULL CHECK(source_column>0),
 UNIQUE(set_group_id,source_order)
) STRICT;
CREATE TABLE catalog_media_entries (
 media_entry_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id)
) STRICT;
CREATE TABLE catalog_relationships (
 relationship_id INTEGER PRIMARY KEY,
 edition_id INTEGER NOT NULL REFERENCES catalog_editions(edition_id)
) STRICT;
CREATE TABLE reported_catalog_relationships (
 relationship_id INTEGER PRIMARY KEY REFERENCES catalog_relationships(relationship_id),
 reported_kind TEXT NOT NULL
) STRICT;
CREATE TABLE hash_values (
 hash_id INTEGER PRIMARY KEY, algorithm TEXT NOT NULL CHECK(algorithm IN ('crc32','md5','sha1','sha256')),
 bytes BLOB NOT NULL,
 CHECK(length(bytes)=CASE algorithm WHEN 'crc32' THEN 4 WHEN 'md5' THEN 16 WHEN 'sha1' THEN 20 ELSE 32 END),
 UNIQUE(algorithm,bytes)
) STRICT;
CREATE TABLE catalog_entry_hashes (
 reported_hash_id INTEGER PRIMARY KEY,
 media_entry_id INTEGER NOT NULL REFERENCES catalog_media_entries(media_entry_id),
 source_hash_field TEXT NOT NULL CHECK(source_hash_field IN ('crc','crc32','md5','sha1','sha256','origin_sha256')),
 field_occurrence INTEGER NOT NULL CHECK(field_occurrence>=0),
 presence TEXT NOT NULL CHECK(presence IN ('empty','invalid','value')),
 hash_scope TEXT NOT NULL CHECK(hash_scope IN ('whole_file','whole_asset','unknown','chd_header_sha1','source_origin','nfo_companion')),
 hash_id INTEGER REFERENCES hash_values(hash_id), reported_text TEXT,
 CHECK((presence='empty' AND hash_id IS NULL AND reported_text IS NULL)
    OR (presence='invalid' AND hash_id IS NULL AND reported_text IS NOT NULL AND reported_text<>'')
    OR (presence='value' AND hash_id IS NOT NULL)),
 UNIQUE(media_entry_id,source_hash_field,field_occurrence)
) STRICT;
CREATE TABLE invalid_catalog_entry_hashes (
 reported_hash_id INTEGER PRIMARY KEY REFERENCES catalog_entry_hashes(reported_hash_id), diagnostic_code TEXT NOT NULL
) STRICT;
CREATE VIEW declared_catalog_hash_text AS
SELECT declaration.reported_hash_id, CASE declaration.presence WHEN 'empty' THEN ''
 WHEN 'invalid' THEN declaration.reported_text
 WHEN 'value' THEN coalesce(declaration.reported_text,lower(hex(value.bytes))) END AS declared_text
FROM catalog_entry_hashes AS declaration LEFT JOIN hash_values AS value USING(hash_id);
.read docs/schema-candidate/mame.sql

CREATE TEMP TABLE mame_field_assertions(label TEXT PRIMARY KEY,ok INTEGER NOT NULL CHECK(ok=1)) STRICT;
INSERT INTO catalog_reading_rules VALUES (1,'constructed-v3','mame','mame-listxml','0.289','constructed','mame-observed-compat-declared-text-v3');
INSERT INTO catalog_editions VALUES (1,1);
INSERT INTO catalog_set_groups VALUES (10,1,'root');
INSERT INTO catalog_source_elements VALUES (100,1,'mame_machine');
INSERT INTO catalog_sets VALUES (100,10,'field-fixture',0,2,3);
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (200,1,'mame_bios_set');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (201,1,'mame_rom');
INSERT INTO catalog_media_entries ("media_entry_id") VALUES (201);
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (202,1,'mame_disk');
INSERT INTO catalog_media_entries ("media_entry_id") VALUES (202);
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (203,1,'mame_device_reference');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (204,1,'mame_sample');
INSERT INTO catalog_media_entries ("media_entry_id") VALUES (204);
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (205,1,'mame_chip');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (206,1,'mame_display');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (207,1,'mame_sound');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (208,1,'mame_input');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (209,1,'mame_input_control');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (210,1,'mame_switch');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (211,1,'mame_switch_location');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (212,1,'mame_switch_value');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (213,1,'mame_switch');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (214,1,'mame_switch_location');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (215,1,'mame_switch_value');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (216,1,'mame_switch_condition');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (217,1,'mame_switch_condition');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (218,1,'mame_switch_value_condition');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (219,1,'mame_switch_value_condition');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (220,1,'mame_port');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (221,1,'mame_analog');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (222,1,'mame_adjuster');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (223,1,'mame_adjuster_condition');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (224,1,'mame_driver');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (225,1,'mame_feature');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (226,1,'mame_device');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (227,1,'mame_device_instance');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (228,1,'mame_device_extension');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (229,1,'mame_slot');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (230,1,'mame_slot_option');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (231,1,'mame_softwarelist_reference');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (232,1,'mame_ram_option');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (233,1,'mame_machine_text');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (234,1,'mame_machine_text');
INSERT INTO catalog_source_elements ("source_element_id","edition_id","element_kind") VALUES (235,1,'mame_machine_text');
INSERT INTO catalog_relationships ("relationship_id","edition_id") VALUES (401,1);
INSERT INTO reported_catalog_relationships ("relationship_id","reported_kind") VALUES (401,'mame_cloneof');
INSERT INTO catalog_relationships ("relationship_id","edition_id") VALUES (402,1);
INSERT INTO reported_catalog_relationships ("relationship_id","reported_kind") VALUES (402,'mame_romof');
INSERT INTO catalog_relationships ("relationship_id","edition_id") VALUES (403,1);
INSERT INTO reported_catalog_relationships ("relationship_id","reported_kind") VALUES (403,'mame_sampleof');
INSERT INTO catalog_relationships ("relationship_id","edition_id") VALUES (404,1);
INSERT INTO reported_catalog_relationships ("relationship_id","reported_kind") VALUES (404,'mame_device_ref');
INSERT INTO catalog_relationships ("relationship_id","edition_id") VALUES (405,1);
INSERT INTO reported_catalog_relationships ("relationship_id","reported_kind") VALUES (405,'mame_rom_merge');
INSERT INTO catalog_relationships ("relationship_id","edition_id") VALUES (406,1);
INSERT INTO reported_catalog_relationships ("relationship_id","reported_kind") VALUES (406,'mame_disk_merge');

-- Concrete source owners, with all optional attributes supplied and every
-- DTD default explicitly supplied as its default value. Both switch kinds
-- and all five actual condition contexts are independent owner instances.
INSERT INTO mame_documents ("edition_id","source_line","source_column","extent_view","extent_start","extent_end","build","debug","debug_specified","mameconfig") VALUES (1,1,1,'transport_decoded_xml_bytes',0,10000,'',0,1,'');
INSERT INTO mame_machines ("set_id","sourcefile","isbios","isbios_specified","isdevice","isdevice_specified","ismechanical","ismechanical_specified","runnable","runnable_specified") VALUES (100,'',0,1,0,1,0,1,1,1);
INSERT INTO mame_bios_sets ("source_element_id","machine_id","source_order","source_line","source_column","name","description","default","default_specified") VALUES (200,100,3,10,3,'','',0,1);
INSERT INTO mame_roms ("media_entry_id","machine_id","source_order","source_line","source_column","name","bios","size_text","region","offset","status","status_specified","optional","optional_specified") VALUES (201,100,4,11,3,'','','18446744073709551615','','+ffffffffffffffff','good',1,0,1);
INSERT INTO mame_disks ("media_entry_id","machine_id","source_order","source_line","source_column","name","region","index","writable","writable_specified","status","status_specified","optional","optional_specified") VALUES (202,100,5,12,3,'','','',0,1,'good',1,0,1);
INSERT INTO mame_device_references ("source_element_id","machine_id","source_order","source_line","source_column","tag","relationship_id","name") VALUES (203,100,6,13,3,'',404,'');
INSERT INTO mame_samples ("media_entry_id","machine_id","source_order","source_line","source_column","name") VALUES (204,100,7,14,3,'');
INSERT INTO mame_chips ("source_element_id","machine_id","source_order","source_line","source_column","name","tag","type","clock") VALUES (205,100,8,15,3,'','','cpu','not-numeric');
INSERT INTO mame_displays ("source_element_id","machine_id","source_order","source_line","source_column","tag","type","rotate","flipx","flipx_specified","width","height","refresh","pixclock","htotal","hbend","hbstart","vtotal","vbend","vbstart") VALUES (206,100,9,16,3,'','raster','0',0,1,'','','','','','','','','','');
INSERT INTO mame_sound ("source_element_id","machine_id","source_order","source_line","source_column","channels") VALUES (207,100,10,17,3,'');
INSERT INTO mame_inputs ("source_element_id","machine_id","source_order","source_line","source_column","service","service_specified","tilt","tilt_specified","players","coins") VALUES (208,100,11,18,3,0,1,0,1,'','');
INSERT INTO mame_input_controls ("source_element_id","input_id","source_order","source_line","source_column","type","player","buttons","minimum","maximum","sensitivity","keydelta","reverse","reverse_specified","ways","ways2","ways3") VALUES (209,208,0,19,3,'','','','','','','',0,1,'','','');
INSERT INTO mame_switches ("source_element_id","machine_id","source_order","source_line","source_column","kind","name","tag","mask") VALUES (210,100,12,20,3,'dipswitch','','','');
INSERT INTO mame_switch_locations ("source_element_id","switch_id","source_order","source_line","source_column","name","number","inverted","inverted_specified") VALUES (211,210,1,21,3,'','',0,1);
INSERT INTO mame_switch_values ("source_element_id","switch_id","source_order","source_line","source_column","name","value","default","default_specified") VALUES (212,210,2,22,3,'','',0,1);
INSERT INTO mame_switches ("source_element_id","machine_id","source_order","source_line","source_column","kind","name","tag","mask") VALUES (213,100,13,23,3,'configuration','','','');
INSERT INTO mame_switch_locations ("source_element_id","switch_id","source_order","source_line","source_column","name","number","inverted","inverted_specified") VALUES (214,213,1,24,3,'','',0,1);
INSERT INTO mame_switch_values ("source_element_id","switch_id","source_order","source_line","source_column","name","value","default","default_specified") VALUES (215,213,2,25,3,'','',0,1);
INSERT INTO mame_switch_conditions ("source_element_id","switch_id","source_order","source_line","source_column","tag","mask","relation","value") VALUES (216,210,0,26,3,'','','eq','');
INSERT INTO mame_switch_conditions ("source_element_id","switch_id","source_order","source_line","source_column","tag","mask","relation","value") VALUES (217,213,0,27,3,'','','eq','');
INSERT INTO mame_switch_value_conditions ("source_element_id","value_id","source_order","source_line","source_column","tag","mask","relation","value") VALUES (218,212,0,28,3,'','','eq','');
INSERT INTO mame_switch_value_conditions ("source_element_id","value_id","source_order","source_line","source_column","tag","mask","relation","value") VALUES (219,215,0,29,3,'','','eq','');
INSERT INTO mame_ports ("source_element_id","machine_id","source_order","source_line","source_column","tag") VALUES (220,100,14,30,3,'');
INSERT INTO mame_analogs ("source_element_id","port_id","source_order","source_line","source_column","mask") VALUES (221,220,0,31,3,'');
INSERT INTO mame_adjusters ("source_element_id","machine_id","source_order","source_line","source_column","name","default") VALUES (222,100,15,32,3,'','source CDATA default');
INSERT INTO mame_adjuster_conditions ("source_element_id","adjuster_id","source_order","source_line","source_column","tag","mask","relation","value") VALUES (223,222,0,33,3,'','','eq','');
INSERT INTO mame_drivers ("source_element_id","machine_id","source_order","source_line","source_column","status","emulation","cocktail","savestate","requiresartwork","requiresartwork_specified","unofficial","unofficial_specified","nosoundhardware","nosoundhardware_specified","incomplete","incomplete_specified") VALUES (224,100,16,34,3,'good','good','good','supported',0,1,0,1,0,1,0,1);
INSERT INTO mame_features ("source_element_id","machine_id","source_order","source_line","source_column","type","status","overall") VALUES (225,100,17,35,3,'protection','unemulated','unemulated');
INSERT INTO mame_devices ("source_element_id","machine_id","source_order","source_line","source_column","type","tag","fixed_image","mandatory","interface") VALUES (226,100,18,36,3,'','','','','');
INSERT INTO mame_device_instances ("source_element_id","device_id","source_order","source_line","source_column","name","briefname") VALUES (227,226,1,37,3,'','');
INSERT INTO mame_device_extensions ("source_element_id","device_id","source_order","source_line","source_column","name") VALUES (228,226,0,38,3,'');
INSERT INTO mame_slots ("source_element_id","machine_id","source_order","source_line","source_column","name") VALUES (229,100,19,39,3,'');
INSERT INTO mame_slot_options ("source_element_id","slot_id","source_order","source_line","source_column","name","devname","default","default_specified") VALUES (230,229,0,40,3,'','',0,1);
INSERT INTO mame_softwarelist_references ("source_element_id","machine_id","source_order","source_line","source_column","tag","name","status","filter") VALUES (231,100,20,41,3,'','','original','');
INSERT INTO mame_ram_options ("source_element_id","machine_id","source_order","source_line","source_column","name","default","text") VALUES (232,100,21,42,3,'','not-a-boolean','128K & direct text');
INSERT INTO mame_machine_text_elements ("source_element_id","machine_id","field_kind","text_value","source_order","source_line","source_column") VALUES (233,100,'description','',0,3,3);
INSERT INTO mame_machine_text_elements ("source_element_id","machine_id","field_kind","text_value","source_order","source_line","source_column") VALUES (234,100,'year','',1,4,3);
INSERT INTO mame_machine_text_elements ("source_element_id","machine_id","field_kind","text_value","source_order","source_line","source_column") VALUES (235,100,'manufacturer','',2,5,3);
INSERT INTO mame_machine_links ("machine_id","link_kind","target_name","relationship_id") VALUES (100,'cloneof','',401);
INSERT INTO mame_machine_links ("machine_id","link_kind","target_name","relationship_id") VALUES (100,'romof','',402);
INSERT INTO mame_machine_links ("machine_id","link_kind","target_name","relationship_id") VALUES (100,'sampleof','',403);
INSERT INTO mame_rom_merges ("media_entry_id","relationship_id","merge_name") VALUES (201,405,'');
INSERT INTO mame_disk_merges ("media_entry_id","relationship_id","merge_name") VALUES (202,406,'');
INSERT INTO mame_machine_compatibility ("set_id","isconsumable") VALUES (100,0);
INSERT INTO mame_rom_compatibility ("media_entry_id","soundonly","dispose","loadflag","value","inverted","ovha","nothread") VALUES (201,0,0,'','',0,'',0);
INSERT INTO mame_disk_compatibility ("media_entry_id","writeable") VALUES (202,0);
INSERT INTO hash_values ("hash_id","algorithm","bytes") VALUES (1,'crc32',x'deadbeef');
INSERT INTO hash_values ("hash_id","algorithm","bytes") VALUES (2,'sha1',x'0123456789abcdef0123456789abcdef01234567');
INSERT INTO hash_values ("hash_id","algorithm","bytes") VALUES (3,'md5',x'0123456789abcdef0123456789abcdef');
INSERT INTO catalog_entry_hashes ("reported_hash_id","media_entry_id","source_hash_field","field_occurrence","presence","hash_scope","hash_id","reported_text") VALUES (501,201,'crc',0,'value','whole_file',1,NULL);
INSERT INTO catalog_entry_hashes ("reported_hash_id","media_entry_id","source_hash_field","field_occurrence","presence","hash_scope","hash_id","reported_text") VALUES (502,201,'sha1',0,'value','whole_file',2,NULL);
INSERT INTO catalog_entry_hashes ("reported_hash_id","media_entry_id","source_hash_field","field_occurrence","presence","hash_scope","hash_id","reported_text") VALUES (504,202,'sha1',0,'value','chd_header_sha1',2,NULL);
INSERT INTO catalog_entry_hashes ("reported_hash_id","media_entry_id","source_hash_field","field_occurrence","presence","hash_scope","hash_id","reported_text") VALUES (503,201,'md5',0,'value','unknown',3,NULL);

-- Every accepted field is inserted into its real closed companion, not a
-- generic expected-code store. Native and compatibility ordinals share a tag.
INSERT INTO mame_document_facts_attribute_positions ("edition_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (1,'build',0,0,1,10);
INSERT INTO mame_document_facts_attribute_positions ("edition_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (1,'debug',0,1,1,22);
INSERT INTO mame_document_facts_attribute_positions ("edition_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (1,'mameconfig',0,2,1,34);
INSERT INTO mame_machines_attribute_positions ("set_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (100,'name',0,0,2,10);
INSERT INTO mame_machines_attribute_positions ("set_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (100,'sourcefile',0,1,2,22);
INSERT INTO mame_machines_attribute_positions ("set_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (100,'isbios',0,2,2,34);
INSERT INTO mame_machines_attribute_positions ("set_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (100,'isdevice',0,3,2,46);
INSERT INTO mame_machines_attribute_positions ("set_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (100,'ismechanical',0,4,2,58);
INSERT INTO mame_machines_attribute_positions ("set_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (100,'runnable',0,5,2,70);
INSERT INTO mame_machines_attribute_positions ("set_id","field_kind","field_occurrence","source_order","source_line","source_column","relationship_id") VALUES (100,'cloneof',0,6,2,82,401);
INSERT INTO mame_machines_attribute_positions ("set_id","field_kind","field_occurrence","source_order","source_line","source_column","relationship_id") VALUES (100,'romof',0,7,2,94,402);
INSERT INTO mame_machines_attribute_positions ("set_id","field_kind","field_occurrence","source_order","source_line","source_column","relationship_id") VALUES (100,'sampleof',0,8,2,106,403);
INSERT INTO mame_bios_sets_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (200,'name',0,0,10,10);
INSERT INTO mame_bios_sets_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (200,'description',0,1,10,22);
INSERT INTO mame_bios_sets_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (200,'default',0,2,10,34);
INSERT INTO mame_rom_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'name',0,0,11,10);
INSERT INTO mame_rom_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'bios',0,1,11,22);
INSERT INTO mame_rom_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'size',0,2,11,34);
INSERT INTO mame_rom_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column","reported_hash_id") VALUES (201,'crc',0,3,11,46,501);
INSERT INTO mame_rom_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column","reported_hash_id") VALUES (201,'sha1',0,4,11,58,502);
INSERT INTO mame_rom_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column","relationship_id") VALUES (201,'merge',0,5,11,70,405);
INSERT INTO mame_rom_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'region',0,6,11,82);
INSERT INTO mame_rom_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'offset',0,7,11,94);
INSERT INTO mame_rom_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'status',0,8,11,106);
INSERT INTO mame_rom_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'optional',0,9,11,118);
INSERT INTO mame_disk_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (202,'name',0,0,12,10);
INSERT INTO mame_disk_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column","reported_hash_id") VALUES (202,'sha1',0,1,12,22,504);
INSERT INTO mame_disk_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column","relationship_id") VALUES (202,'merge',0,2,12,34,406);
INSERT INTO mame_disk_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (202,'region',0,3,12,46);
INSERT INTO mame_disk_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (202,'index',0,4,12,58);
INSERT INTO mame_disk_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (202,'writable',0,5,12,70);
INSERT INTO mame_disk_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (202,'status',0,6,12,82);
INSERT INTO mame_disk_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (202,'optional',0,7,12,94);
INSERT INTO mame_device_references_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (203,'tag',0,0,13,10);
INSERT INTO mame_device_references_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column","relationship_id") VALUES (203,'name',0,1,13,22,404);
INSERT INTO mame_samples_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (204,'name',0,0,14,10);
INSERT INTO mame_machine_chips_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (205,'name',0,0,15,10);
INSERT INTO mame_machine_chips_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (205,'tag',0,1,15,22);
INSERT INTO mame_machine_chips_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (205,'type',0,2,15,34);
INSERT INTO mame_machine_chips_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (205,'clock',0,3,15,46);
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'tag',0,0,16,10);
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'type',0,1,16,22);
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'rotate',0,2,16,34);
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'flipx',0,3,16,46);
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'width',0,4,16,58);
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'height',0,5,16,70);
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'refresh',0,6,16,82);
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'pixclock',0,7,16,94);
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'htotal',0,8,16,106);
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'hbend',0,9,16,118);
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'hbstart',0,10,16,130);
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'vtotal',0,11,16,142);
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'vbend',0,12,16,154);
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'vbstart',0,13,16,166);
INSERT INTO mame_machine_sounds_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (207,'channels',0,0,17,10);
INSERT INTO mame_machine_inputs_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (208,'service',0,0,18,10);
INSERT INTO mame_machine_inputs_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (208,'tilt',0,1,18,22);
INSERT INTO mame_machine_inputs_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (208,'players',0,2,18,34);
INSERT INTO mame_machine_inputs_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (208,'coins',0,3,18,46);
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'type',0,0,19,10);
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'player',0,1,19,22);
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'buttons',0,2,19,34);
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'minimum',0,3,19,46);
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'maximum',0,4,19,58);
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'sensitivity',0,5,19,70);
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'keydelta',0,6,19,82);
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'reverse',0,7,19,94);
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'ways',0,8,19,106);
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'ways2',0,9,19,118);
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'ways3',0,10,19,130);
INSERT INTO machine_switches_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (210,'name',0,0,20,10);
INSERT INTO machine_switches_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (210,'tag',0,1,20,22);
INSERT INTO machine_switches_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (210,'mask',0,2,20,34);
INSERT INTO machine_switch_locations_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (211,'name',0,0,21,10);
INSERT INTO machine_switch_locations_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (211,'number',0,1,21,22);
INSERT INTO machine_switch_locations_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (211,'inverted',0,2,21,34);
INSERT INTO machine_switch_values_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (212,'name',0,0,22,10);
INSERT INTO machine_switch_values_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (212,'value',0,1,22,22);
INSERT INTO machine_switch_values_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (212,'default',0,2,22,34);
INSERT INTO machine_switches_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (213,'name',0,0,23,10);
INSERT INTO machine_switches_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (213,'tag',0,1,23,22);
INSERT INTO machine_switches_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (213,'mask',0,2,23,34);
INSERT INTO machine_switch_locations_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (214,'name',0,0,24,10);
INSERT INTO machine_switch_locations_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (214,'number',0,1,24,22);
INSERT INTO machine_switch_locations_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (214,'inverted',0,2,24,34);
INSERT INTO machine_switch_values_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (215,'name',0,0,25,10);
INSERT INTO machine_switch_values_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (215,'value',0,1,25,22);
INSERT INTO machine_switch_values_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (215,'default',0,2,25,34);
INSERT INTO machine_switch_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (216,'tag',0,0,26,10);
INSERT INTO machine_switch_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (216,'mask',0,1,26,22);
INSERT INTO machine_switch_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (216,'relation',0,2,26,34);
INSERT INTO machine_switch_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (216,'value',0,3,26,46);
INSERT INTO machine_switch_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (217,'tag',0,0,27,10);
INSERT INTO machine_switch_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (217,'mask',0,1,27,22);
INSERT INTO machine_switch_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (217,'relation',0,2,27,34);
INSERT INTO machine_switch_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (217,'value',0,3,27,46);
INSERT INTO machine_switch_value_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (218,'tag',0,0,28,10);
INSERT INTO machine_switch_value_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (218,'mask',0,1,28,22);
INSERT INTO machine_switch_value_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (218,'relation',0,2,28,34);
INSERT INTO machine_switch_value_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (218,'value',0,3,28,46);
INSERT INTO machine_switch_value_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (219,'tag',0,0,29,10);
INSERT INTO machine_switch_value_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (219,'mask',0,1,29,22);
INSERT INTO machine_switch_value_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (219,'relation',0,2,29,34);
INSERT INTO machine_switch_value_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (219,'value',0,3,29,46);
INSERT INTO mame_machine_ports_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (220,'tag',0,0,30,10);
INSERT INTO mame_machine_analogs_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (221,'mask',0,0,31,10);
INSERT INTO mame_machine_adjusters_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (222,'name',0,0,32,10);
INSERT INTO mame_machine_adjusters_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (222,'default',0,1,32,22);
INSERT INTO mame_machine_adjuster_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (223,'tag',0,0,33,10);
INSERT INTO mame_machine_adjuster_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (223,'mask',0,1,33,22);
INSERT INTO mame_machine_adjuster_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (223,'relation',0,2,33,34);
INSERT INTO mame_machine_adjuster_conditions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (223,'value',0,3,33,46);
INSERT INTO mame_machine_drivers_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (224,'status',0,0,34,10);
INSERT INTO mame_machine_drivers_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (224,'emulation',0,1,34,22);
INSERT INTO mame_machine_drivers_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (224,'cocktail',0,2,34,34);
INSERT INTO mame_machine_drivers_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (224,'savestate',0,3,34,46);
INSERT INTO mame_machine_drivers_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (224,'requiresartwork',0,4,34,58);
INSERT INTO mame_machine_drivers_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (224,'unofficial',0,5,34,70);
INSERT INTO mame_machine_drivers_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (224,'nosoundhardware',0,6,34,82);
INSERT INTO mame_machine_drivers_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (224,'incomplete',0,7,34,94);
INSERT INTO mame_machine_features_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (225,'type',0,0,35,10);
INSERT INTO mame_machine_features_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (225,'status',0,1,35,22);
INSERT INTO mame_machine_features_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (225,'overall',0,2,35,34);
INSERT INTO mame_machine_devices_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (226,'type',0,0,36,10);
INSERT INTO mame_machine_devices_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (226,'tag',0,1,36,22);
INSERT INTO mame_machine_devices_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (226,'fixed_image',0,2,36,34);
INSERT INTO mame_machine_devices_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (226,'mandatory',0,3,36,46);
INSERT INTO mame_machine_devices_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (226,'interface',0,4,36,58);
INSERT INTO mame_machine_device_instances_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (227,'name',0,0,37,10);
INSERT INTO mame_machine_device_instances_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (227,'briefname',0,1,37,22);
INSERT INTO mame_machine_device_extensions_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (228,'name',0,0,38,10);
INSERT INTO mame_machine_slots_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (229,'name',0,0,39,10);
INSERT INTO mame_machine_slot_options_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (230,'name',0,0,40,10);
INSERT INTO mame_machine_slot_options_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (230,'devname',0,1,40,22);
INSERT INTO mame_machine_slot_options_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (230,'default',0,2,40,34);
INSERT INTO mame_machine_software_lists_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (231,'tag',0,0,41,10);
INSERT INTO mame_machine_software_lists_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (231,'name',0,1,41,22);
INSERT INTO mame_machine_software_lists_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (231,'status',0,2,41,34);
INSERT INTO mame_machine_software_lists_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (231,'filter',0,3,41,46);
INSERT INTO mame_machine_ram_options_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (232,'name',0,0,42,10);
INSERT INTO mame_machine_ram_options_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (232,'default',0,1,42,22);
INSERT INTO mame_machine_compatibility_attribute_positions ("set_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (100,'isconsumable',0,9,2,118);
INSERT INTO mame_rom_compatibility_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column","reported_hash_id") VALUES (201,'md5',0,10,11,130,503);
INSERT INTO mame_rom_compatibility_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'soundonly',0,11,11,142);
INSERT INTO mame_rom_compatibility_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'dispose',0,12,11,154);
INSERT INTO mame_rom_compatibility_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'loadflag',0,13,11,166);
INSERT INTO mame_rom_compatibility_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'value',0,14,11,178);
INSERT INTO mame_rom_compatibility_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'inverted',0,15,11,190);
INSERT INTO mame_rom_compatibility_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'ovha',0,16,11,202);
INSERT INTO mame_rom_compatibility_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'nothread',0,17,11,214);
INSERT INTO mame_disk_compatibility_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (202,'writeable',0,8,12,106);

-- Positive value AND position assertions for every ledger context.
INSERT INTO mame_field_assertions VALUES ('field:mame:build',(EXISTS(SELECT 1 FROM mame_documents AS value WHERE value."edition_id"=1 AND value."build" IS '') AND EXISTS(SELECT 1 FROM mame_document_facts_attribute_positions AS position WHERE position."edition_id"=1 AND position.field_kind='build' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:mame:debug',(EXISTS(SELECT 1 FROM mame_documents AS value WHERE value."edition_id"=1 AND value."debug" IS 0 AND value."debug_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_document_facts_attribute_positions AS position WHERE position."edition_id"=1 AND position.field_kind='debug' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:mame:mameconfig',(EXISTS(SELECT 1 FROM mame_documents AS value WHERE value."edition_id"=1 AND value."mameconfig" IS '') AND EXISTS(SELECT 1 FROM mame_document_facts_attribute_positions AS position WHERE position."edition_id"=1 AND position.field_kind='mameconfig' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:machine:name',(EXISTS(SELECT 1 FROM catalog_sets AS value WHERE value."set_id"=100 AND value."set_name" IS 'field-fixture') AND EXISTS(SELECT 1 FROM mame_machines_attribute_positions AS position WHERE position."set_id"=100 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:machine:sourcefile',(EXISTS(SELECT 1 FROM mame_machines AS value WHERE value."set_id"=100 AND value."sourcefile" IS '') AND EXISTS(SELECT 1 FROM mame_machines_attribute_positions AS position WHERE position."set_id"=100 AND position.field_kind='sourcefile' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:machine:isbios',(EXISTS(SELECT 1 FROM mame_machines AS value WHERE value."set_id"=100 AND value."isbios" IS 0 AND value."isbios_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_machines_attribute_positions AS position WHERE position."set_id"=100 AND position.field_kind='isbios' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:machine:isdevice',(EXISTS(SELECT 1 FROM mame_machines AS value WHERE value."set_id"=100 AND value."isdevice" IS 0 AND value."isdevice_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_machines_attribute_positions AS position WHERE position."set_id"=100 AND position.field_kind='isdevice' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:machine:ismechanical',(EXISTS(SELECT 1 FROM mame_machines AS value WHERE value."set_id"=100 AND value."ismechanical" IS 0 AND value."ismechanical_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_machines_attribute_positions AS position WHERE position."set_id"=100 AND position.field_kind='ismechanical' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:machine:runnable',(EXISTS(SELECT 1 FROM mame_machines AS value WHERE value."set_id"=100 AND value."runnable" IS 1 AND value."runnable_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_machines_attribute_positions AS position WHERE position."set_id"=100 AND position.field_kind='runnable' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:machine:cloneof',(EXISTS(SELECT 1 FROM mame_machine_links AS value WHERE value.machine_id=100 AND value.link_kind='cloneof' AND value."target_name" IS '') AND EXISTS(SELECT 1 FROM mame_machines_attribute_positions AS position WHERE position."set_id"=100 AND position.field_kind='cloneof' AND position.field_occurrence=0 AND position.relationship_id=401)));
INSERT INTO mame_field_assertions VALUES ('field:machine:romof',(EXISTS(SELECT 1 FROM mame_machine_links AS value WHERE value.machine_id=100 AND value.link_kind='romof' AND value."target_name" IS '') AND EXISTS(SELECT 1 FROM mame_machines_attribute_positions AS position WHERE position."set_id"=100 AND position.field_kind='romof' AND position.field_occurrence=0 AND position.relationship_id=402)));
INSERT INTO mame_field_assertions VALUES ('field:machine:sampleof',(EXISTS(SELECT 1 FROM mame_machine_links AS value WHERE value.machine_id=100 AND value.link_kind='sampleof' AND value."target_name" IS '') AND EXISTS(SELECT 1 FROM mame_machines_attribute_positions AS position WHERE position."set_id"=100 AND position.field_kind='sampleof' AND position.field_occurrence=0 AND position.relationship_id=403)));
INSERT INTO mame_field_assertions VALUES ('field:biosset:name',(EXISTS(SELECT 1 FROM mame_bios_sets AS value WHERE value."source_element_id"=200 AND value."name" IS '') AND EXISTS(SELECT 1 FROM mame_bios_sets_attribute_positions AS position WHERE position."source_element_id"=200 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:biosset:description',(EXISTS(SELECT 1 FROM mame_bios_sets AS value WHERE value."source_element_id"=200 AND value."description" IS '') AND EXISTS(SELECT 1 FROM mame_bios_sets_attribute_positions AS position WHERE position."source_element_id"=200 AND position.field_kind='description' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:biosset:default',(EXISTS(SELECT 1 FROM mame_bios_sets AS value WHERE value."source_element_id"=200 AND value."default" IS 0 AND value."default_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_bios_sets_attribute_positions AS position WHERE position."source_element_id"=200 AND position.field_kind='default' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:rom:name',(EXISTS(SELECT 1 FROM mame_roms AS value WHERE value."media_entry_id"=201 AND value."name" IS '') AND EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=201 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:rom:bios',(EXISTS(SELECT 1 FROM mame_roms AS value WHERE value."media_entry_id"=201 AND value."bios" IS '') AND EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=201 AND position.field_kind='bios' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:rom:size',(EXISTS(SELECT 1 FROM mame_roms AS value WHERE value."media_entry_id"=201 AND value."size_text" IS '18446744073709551615') AND EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=201 AND position.field_kind='size' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:rom:crc',(EXISTS(SELECT 1 FROM catalog_entry_hashes AS value JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE value.reported_hash_id=501 AND value.presence='value' AND declared_text='deadbeef') AND EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=201 AND position.field_kind='crc' AND position.field_occurrence=0 AND position.reported_hash_id=501)));
INSERT INTO mame_field_assertions VALUES ('field:rom:sha1',(EXISTS(SELECT 1 FROM catalog_entry_hashes AS value JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE value.reported_hash_id=502 AND value.presence='value' AND declared_text='0123456789abcdef0123456789abcdef01234567') AND EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=201 AND position.field_kind='sha1' AND position.field_occurrence=0 AND position.reported_hash_id=502)));
INSERT INTO mame_field_assertions VALUES ('field:rom:merge',(EXISTS(SELECT 1 FROM mame_rom_merges AS value WHERE value."media_entry_id"=201 AND value."merge_name" IS '') AND EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=201 AND position.field_kind='merge' AND position.field_occurrence=0 AND position.relationship_id=405)));
INSERT INTO mame_field_assertions VALUES ('field:rom:region',(EXISTS(SELECT 1 FROM mame_roms AS value WHERE value."media_entry_id"=201 AND value."region" IS '') AND EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=201 AND position.field_kind='region' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:rom:offset',(EXISTS(SELECT 1 FROM mame_roms AS value WHERE value."media_entry_id"=201 AND value."offset" IS '+ffffffffffffffff') AND EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=201 AND position.field_kind='offset' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:rom:status',(EXISTS(SELECT 1 FROM mame_roms AS value WHERE value."media_entry_id"=201 AND value."status" IS 'good' AND value."status_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=201 AND position.field_kind='status' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:rom:optional',(EXISTS(SELECT 1 FROM mame_roms AS value WHERE value."media_entry_id"=201 AND value."optional" IS 0 AND value."optional_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=201 AND position.field_kind='optional' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:disk:name',(EXISTS(SELECT 1 FROM mame_disks AS value WHERE value."media_entry_id"=202 AND value."name" IS '') AND EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions AS position WHERE position."media_entry_id"=202 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:disk:sha1',(EXISTS(SELECT 1 FROM catalog_entry_hashes AS value JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE value.reported_hash_id=504 AND value.presence='value' AND declared_text='0123456789abcdef0123456789abcdef01234567') AND EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions AS position WHERE position."media_entry_id"=202 AND position.field_kind='sha1' AND position.field_occurrence=0 AND position.reported_hash_id=504)));
INSERT INTO mame_field_assertions VALUES ('field:disk:merge',(EXISTS(SELECT 1 FROM mame_disk_merges AS value WHERE value."media_entry_id"=202 AND value."merge_name" IS '') AND EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions AS position WHERE position."media_entry_id"=202 AND position.field_kind='merge' AND position.field_occurrence=0 AND position.relationship_id=406)));
INSERT INTO mame_field_assertions VALUES ('field:disk:region',(EXISTS(SELECT 1 FROM mame_disks AS value WHERE value."media_entry_id"=202 AND value."region" IS '') AND EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions AS position WHERE position."media_entry_id"=202 AND position.field_kind='region' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:disk:index',(EXISTS(SELECT 1 FROM mame_disks AS value WHERE value."media_entry_id"=202 AND value."index" IS '') AND EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions AS position WHERE position."media_entry_id"=202 AND position.field_kind='index' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:disk:writable',(EXISTS(SELECT 1 FROM mame_disks AS value WHERE value."media_entry_id"=202 AND value."writable" IS 0 AND value."writable_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions AS position WHERE position."media_entry_id"=202 AND position.field_kind='writable' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:disk:status',(EXISTS(SELECT 1 FROM mame_disks AS value WHERE value."media_entry_id"=202 AND value."status" IS 'good' AND value."status_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions AS position WHERE position."media_entry_id"=202 AND position.field_kind='status' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:disk:optional',(EXISTS(SELECT 1 FROM mame_disks AS value WHERE value."media_entry_id"=202 AND value."optional" IS 0 AND value."optional_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions AS position WHERE position."media_entry_id"=202 AND position.field_kind='optional' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:device_ref:tag',(EXISTS(SELECT 1 FROM mame_device_references AS value WHERE value."source_element_id"=203 AND value."tag" IS '') AND EXISTS(SELECT 1 FROM mame_device_references_attribute_positions AS position WHERE position."source_element_id"=203 AND position.field_kind='tag' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:device_ref:name',(EXISTS(SELECT 1 FROM mame_device_references AS value WHERE value."source_element_id"=203 AND value."name" IS '') AND EXISTS(SELECT 1 FROM mame_device_references_attribute_positions AS position WHERE position."source_element_id"=203 AND position.field_kind='name' AND position.field_occurrence=0 AND position.relationship_id=404)));
INSERT INTO mame_field_assertions VALUES ('field:sample:name',(EXISTS(SELECT 1 FROM mame_samples AS value WHERE value."media_entry_id"=204 AND value."name" IS '') AND EXISTS(SELECT 1 FROM mame_samples_attribute_positions AS position WHERE position."media_entry_id"=204 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:chip:name',(EXISTS(SELECT 1 FROM mame_chips AS value WHERE value."source_element_id"=205 AND value."name" IS '') AND EXISTS(SELECT 1 FROM mame_machine_chips_attribute_positions AS position WHERE position."source_element_id"=205 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:chip:tag',(EXISTS(SELECT 1 FROM mame_chips AS value WHERE value."source_element_id"=205 AND value."tag" IS '') AND EXISTS(SELECT 1 FROM mame_machine_chips_attribute_positions AS position WHERE position."source_element_id"=205 AND position.field_kind='tag' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:chip:type',(EXISTS(SELECT 1 FROM mame_chips AS value WHERE value."source_element_id"=205 AND value."type" IS 'cpu') AND EXISTS(SELECT 1 FROM mame_machine_chips_attribute_positions AS position WHERE position."source_element_id"=205 AND position.field_kind='type' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:chip:clock',(EXISTS(SELECT 1 FROM mame_chips AS value WHERE value."source_element_id"=205 AND value."clock" IS 'not-numeric') AND EXISTS(SELECT 1 FROM mame_machine_chips_attribute_positions AS position WHERE position."source_element_id"=205 AND position.field_kind='clock' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:display:tag',(EXISTS(SELECT 1 FROM mame_displays AS value WHERE value."source_element_id"=206 AND value."tag" IS '') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=206 AND position.field_kind='tag' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:display:type',(EXISTS(SELECT 1 FROM mame_displays AS value WHERE value."source_element_id"=206 AND value."type" IS 'raster') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=206 AND position.field_kind='type' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:display:rotate',(EXISTS(SELECT 1 FROM mame_displays AS value WHERE value."source_element_id"=206 AND value."rotate" IS '0') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=206 AND position.field_kind='rotate' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:display:flipx',(EXISTS(SELECT 1 FROM mame_displays AS value WHERE value."source_element_id"=206 AND value."flipx" IS 0 AND value."flipx_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=206 AND position.field_kind='flipx' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:display:width',(EXISTS(SELECT 1 FROM mame_displays AS value WHERE value."source_element_id"=206 AND value."width" IS '') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=206 AND position.field_kind='width' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:display:height',(EXISTS(SELECT 1 FROM mame_displays AS value WHERE value."source_element_id"=206 AND value."height" IS '') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=206 AND position.field_kind='height' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:display:refresh',(EXISTS(SELECT 1 FROM mame_displays AS value WHERE value."source_element_id"=206 AND value."refresh" IS '') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=206 AND position.field_kind='refresh' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:display:pixclock',(EXISTS(SELECT 1 FROM mame_displays AS value WHERE value."source_element_id"=206 AND value."pixclock" IS '') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=206 AND position.field_kind='pixclock' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:display:htotal',(EXISTS(SELECT 1 FROM mame_displays AS value WHERE value."source_element_id"=206 AND value."htotal" IS '') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=206 AND position.field_kind='htotal' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:display:hbend',(EXISTS(SELECT 1 FROM mame_displays AS value WHERE value."source_element_id"=206 AND value."hbend" IS '') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=206 AND position.field_kind='hbend' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:display:hbstart',(EXISTS(SELECT 1 FROM mame_displays AS value WHERE value."source_element_id"=206 AND value."hbstart" IS '') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=206 AND position.field_kind='hbstart' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:display:vtotal',(EXISTS(SELECT 1 FROM mame_displays AS value WHERE value."source_element_id"=206 AND value."vtotal" IS '') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=206 AND position.field_kind='vtotal' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:display:vbend',(EXISTS(SELECT 1 FROM mame_displays AS value WHERE value."source_element_id"=206 AND value."vbend" IS '') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=206 AND position.field_kind='vbend' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:display:vbstart',(EXISTS(SELECT 1 FROM mame_displays AS value WHERE value."source_element_id"=206 AND value."vbstart" IS '') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=206 AND position.field_kind='vbstart' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:sound:channels',(EXISTS(SELECT 1 FROM mame_sound AS value WHERE value."source_element_id"=207 AND value."channels" IS '') AND EXISTS(SELECT 1 FROM mame_machine_sounds_attribute_positions AS position WHERE position."source_element_id"=207 AND position.field_kind='channels' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:input:service',(EXISTS(SELECT 1 FROM mame_inputs AS value WHERE value."source_element_id"=208 AND value."service" IS 0 AND value."service_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_machine_inputs_attribute_positions AS position WHERE position."source_element_id"=208 AND position.field_kind='service' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:input:tilt',(EXISTS(SELECT 1 FROM mame_inputs AS value WHERE value."source_element_id"=208 AND value."tilt" IS 0 AND value."tilt_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_machine_inputs_attribute_positions AS position WHERE position."source_element_id"=208 AND position.field_kind='tilt' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:input:players',(EXISTS(SELECT 1 FROM mame_inputs AS value WHERE value."source_element_id"=208 AND value."players" IS '') AND EXISTS(SELECT 1 FROM mame_machine_inputs_attribute_positions AS position WHERE position."source_element_id"=208 AND position.field_kind='players' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:input:coins',(EXISTS(SELECT 1 FROM mame_inputs AS value WHERE value."source_element_id"=208 AND value."coins" IS '') AND EXISTS(SELECT 1 FROM mame_machine_inputs_attribute_positions AS position WHERE position."source_element_id"=208 AND position.field_kind='coins' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:control:type',(EXISTS(SELECT 1 FROM mame_input_controls AS value WHERE value."source_element_id"=209 AND value."type" IS '') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=209 AND position.field_kind='type' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:control:player',(EXISTS(SELECT 1 FROM mame_input_controls AS value WHERE value."source_element_id"=209 AND value."player" IS '') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=209 AND position.field_kind='player' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:control:buttons',(EXISTS(SELECT 1 FROM mame_input_controls AS value WHERE value."source_element_id"=209 AND value."buttons" IS '') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=209 AND position.field_kind='buttons' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:control:minimum',(EXISTS(SELECT 1 FROM mame_input_controls AS value WHERE value."source_element_id"=209 AND value."minimum" IS '') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=209 AND position.field_kind='minimum' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:control:maximum',(EXISTS(SELECT 1 FROM mame_input_controls AS value WHERE value."source_element_id"=209 AND value."maximum" IS '') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=209 AND position.field_kind='maximum' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:control:sensitivity',(EXISTS(SELECT 1 FROM mame_input_controls AS value WHERE value."source_element_id"=209 AND value."sensitivity" IS '') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=209 AND position.field_kind='sensitivity' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:control:keydelta',(EXISTS(SELECT 1 FROM mame_input_controls AS value WHERE value."source_element_id"=209 AND value."keydelta" IS '') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=209 AND position.field_kind='keydelta' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:control:reverse',(EXISTS(SELECT 1 FROM mame_input_controls AS value WHERE value."source_element_id"=209 AND value."reverse" IS 0 AND value."reverse_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=209 AND position.field_kind='reverse' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:control:ways',(EXISTS(SELECT 1 FROM mame_input_controls AS value WHERE value."source_element_id"=209 AND value."ways" IS '') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=209 AND position.field_kind='ways' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:control:ways2',(EXISTS(SELECT 1 FROM mame_input_controls AS value WHERE value."source_element_id"=209 AND value."ways2" IS '') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=209 AND position.field_kind='ways2' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:control:ways3',(EXISTS(SELECT 1 FROM mame_input_controls AS value WHERE value."source_element_id"=209 AND value."ways3" IS '') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=209 AND position.field_kind='ways3' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:dipswitch:name',(EXISTS(SELECT 1 FROM mame_switches AS value WHERE value."source_element_id"=210 AND value."name" IS '') AND EXISTS(SELECT 1 FROM machine_switches_attribute_positions AS position WHERE position."source_element_id"=210 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:dipswitch:tag',(EXISTS(SELECT 1 FROM mame_switches AS value WHERE value."source_element_id"=210 AND value."tag" IS '') AND EXISTS(SELECT 1 FROM machine_switches_attribute_positions AS position WHERE position."source_element_id"=210 AND position.field_kind='tag' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:dipswitch:mask',(EXISTS(SELECT 1 FROM mame_switches AS value WHERE value."source_element_id"=210 AND value."mask" IS '') AND EXISTS(SELECT 1 FROM machine_switches_attribute_positions AS position WHERE position."source_element_id"=210 AND position.field_kind='mask' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:diplocation:name',(EXISTS(SELECT 1 FROM mame_switch_locations AS value WHERE value."source_element_id"=211 AND value."name" IS '') AND EXISTS(SELECT 1 FROM machine_switch_locations_attribute_positions AS position WHERE position."source_element_id"=211 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:diplocation:number',(EXISTS(SELECT 1 FROM mame_switch_locations AS value WHERE value."source_element_id"=211 AND value."number" IS '') AND EXISTS(SELECT 1 FROM machine_switch_locations_attribute_positions AS position WHERE position."source_element_id"=211 AND position.field_kind='number' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:diplocation:inverted',(EXISTS(SELECT 1 FROM mame_switch_locations AS value WHERE value."source_element_id"=211 AND value."inverted" IS 0 AND value."inverted_specified" IS 1) AND EXISTS(SELECT 1 FROM machine_switch_locations_attribute_positions AS position WHERE position."source_element_id"=211 AND position.field_kind='inverted' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:dipvalue:name',(EXISTS(SELECT 1 FROM mame_switch_values AS value WHERE value."source_element_id"=212 AND value."name" IS '') AND EXISTS(SELECT 1 FROM machine_switch_values_attribute_positions AS position WHERE position."source_element_id"=212 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:dipvalue:value',(EXISTS(SELECT 1 FROM mame_switch_values AS value WHERE value."source_element_id"=212 AND value."value" IS '') AND EXISTS(SELECT 1 FROM machine_switch_values_attribute_positions AS position WHERE position."source_element_id"=212 AND position.field_kind='value' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:dipvalue:default',(EXISTS(SELECT 1 FROM mame_switch_values AS value WHERE value."source_element_id"=212 AND value."default" IS 0 AND value."default_specified" IS 1) AND EXISTS(SELECT 1 FROM machine_switch_values_attribute_positions AS position WHERE position."source_element_id"=212 AND position.field_kind='default' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:configuration:name',(EXISTS(SELECT 1 FROM mame_switches AS value WHERE value."source_element_id"=213 AND value."name" IS '') AND EXISTS(SELECT 1 FROM machine_switches_attribute_positions AS position WHERE position."source_element_id"=213 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:configuration:tag',(EXISTS(SELECT 1 FROM mame_switches AS value WHERE value."source_element_id"=213 AND value."tag" IS '') AND EXISTS(SELECT 1 FROM machine_switches_attribute_positions AS position WHERE position."source_element_id"=213 AND position.field_kind='tag' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:configuration:mask',(EXISTS(SELECT 1 FROM mame_switches AS value WHERE value."source_element_id"=213 AND value."mask" IS '') AND EXISTS(SELECT 1 FROM machine_switches_attribute_positions AS position WHERE position."source_element_id"=213 AND position.field_kind='mask' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:conflocation:name',(EXISTS(SELECT 1 FROM mame_switch_locations AS value WHERE value."source_element_id"=214 AND value."name" IS '') AND EXISTS(SELECT 1 FROM machine_switch_locations_attribute_positions AS position WHERE position."source_element_id"=214 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:conflocation:number',(EXISTS(SELECT 1 FROM mame_switch_locations AS value WHERE value."source_element_id"=214 AND value."number" IS '') AND EXISTS(SELECT 1 FROM machine_switch_locations_attribute_positions AS position WHERE position."source_element_id"=214 AND position.field_kind='number' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:conflocation:inverted',(EXISTS(SELECT 1 FROM mame_switch_locations AS value WHERE value."source_element_id"=214 AND value."inverted" IS 0 AND value."inverted_specified" IS 1) AND EXISTS(SELECT 1 FROM machine_switch_locations_attribute_positions AS position WHERE position."source_element_id"=214 AND position.field_kind='inverted' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:confsetting:name',(EXISTS(SELECT 1 FROM mame_switch_values AS value WHERE value."source_element_id"=215 AND value."name" IS '') AND EXISTS(SELECT 1 FROM machine_switch_values_attribute_positions AS position WHERE position."source_element_id"=215 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:confsetting:value',(EXISTS(SELECT 1 FROM mame_switch_values AS value WHERE value."source_element_id"=215 AND value."value" IS '') AND EXISTS(SELECT 1 FROM machine_switch_values_attribute_positions AS position WHERE position."source_element_id"=215 AND position.field_kind='value' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:confsetting:default',(EXISTS(SELECT 1 FROM mame_switch_values AS value WHERE value."source_element_id"=215 AND value."default" IS 0 AND value."default_specified" IS 1) AND EXISTS(SELECT 1 FROM machine_switch_values_attribute_positions AS position WHERE position."source_element_id"=215 AND position.field_kind='default' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:dipswitch.condition:tag',(EXISTS(SELECT 1 FROM mame_switch_conditions AS value WHERE value."source_element_id"=216 AND value."tag" IS '') AND EXISTS(SELECT 1 FROM machine_switch_conditions_attribute_positions AS position WHERE position."source_element_id"=216 AND position.field_kind='tag' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:dipswitch.condition:mask',(EXISTS(SELECT 1 FROM mame_switch_conditions AS value WHERE value."source_element_id"=216 AND value."mask" IS '') AND EXISTS(SELECT 1 FROM machine_switch_conditions_attribute_positions AS position WHERE position."source_element_id"=216 AND position.field_kind='mask' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:dipswitch.condition:relation',(EXISTS(SELECT 1 FROM mame_switch_conditions AS value WHERE value."source_element_id"=216 AND value."relation" IS 'eq') AND EXISTS(SELECT 1 FROM machine_switch_conditions_attribute_positions AS position WHERE position."source_element_id"=216 AND position.field_kind='relation' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:dipswitch.condition:value',(EXISTS(SELECT 1 FROM mame_switch_conditions AS value WHERE value."source_element_id"=216 AND value."value" IS '') AND EXISTS(SELECT 1 FROM machine_switch_conditions_attribute_positions AS position WHERE position."source_element_id"=216 AND position.field_kind='value' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:configuration.condition:tag',(EXISTS(SELECT 1 FROM mame_switch_conditions AS value WHERE value."source_element_id"=217 AND value."tag" IS '') AND EXISTS(SELECT 1 FROM machine_switch_conditions_attribute_positions AS position WHERE position."source_element_id"=217 AND position.field_kind='tag' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:configuration.condition:mask',(EXISTS(SELECT 1 FROM mame_switch_conditions AS value WHERE value."source_element_id"=217 AND value."mask" IS '') AND EXISTS(SELECT 1 FROM machine_switch_conditions_attribute_positions AS position WHERE position."source_element_id"=217 AND position.field_kind='mask' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:configuration.condition:relation',(EXISTS(SELECT 1 FROM mame_switch_conditions AS value WHERE value."source_element_id"=217 AND value."relation" IS 'eq') AND EXISTS(SELECT 1 FROM machine_switch_conditions_attribute_positions AS position WHERE position."source_element_id"=217 AND position.field_kind='relation' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:configuration.condition:value',(EXISTS(SELECT 1 FROM mame_switch_conditions AS value WHERE value."source_element_id"=217 AND value."value" IS '') AND EXISTS(SELECT 1 FROM machine_switch_conditions_attribute_positions AS position WHERE position."source_element_id"=217 AND position.field_kind='value' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:dipvalue.condition:tag',(EXISTS(SELECT 1 FROM mame_switch_value_conditions AS value WHERE value."source_element_id"=218 AND value."tag" IS '') AND EXISTS(SELECT 1 FROM machine_switch_value_conditions_attribute_positions AS position WHERE position."source_element_id"=218 AND position.field_kind='tag' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:dipvalue.condition:mask',(EXISTS(SELECT 1 FROM mame_switch_value_conditions AS value WHERE value."source_element_id"=218 AND value."mask" IS '') AND EXISTS(SELECT 1 FROM machine_switch_value_conditions_attribute_positions AS position WHERE position."source_element_id"=218 AND position.field_kind='mask' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:dipvalue.condition:relation',(EXISTS(SELECT 1 FROM mame_switch_value_conditions AS value WHERE value."source_element_id"=218 AND value."relation" IS 'eq') AND EXISTS(SELECT 1 FROM machine_switch_value_conditions_attribute_positions AS position WHERE position."source_element_id"=218 AND position.field_kind='relation' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:dipvalue.condition:value',(EXISTS(SELECT 1 FROM mame_switch_value_conditions AS value WHERE value."source_element_id"=218 AND value."value" IS '') AND EXISTS(SELECT 1 FROM machine_switch_value_conditions_attribute_positions AS position WHERE position."source_element_id"=218 AND position.field_kind='value' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:confsetting.condition:tag',(EXISTS(SELECT 1 FROM mame_switch_value_conditions AS value WHERE value."source_element_id"=219 AND value."tag" IS '') AND EXISTS(SELECT 1 FROM machine_switch_value_conditions_attribute_positions AS position WHERE position."source_element_id"=219 AND position.field_kind='tag' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:confsetting.condition:mask',(EXISTS(SELECT 1 FROM mame_switch_value_conditions AS value WHERE value."source_element_id"=219 AND value."mask" IS '') AND EXISTS(SELECT 1 FROM machine_switch_value_conditions_attribute_positions AS position WHERE position."source_element_id"=219 AND position.field_kind='mask' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:confsetting.condition:relation',(EXISTS(SELECT 1 FROM mame_switch_value_conditions AS value WHERE value."source_element_id"=219 AND value."relation" IS 'eq') AND EXISTS(SELECT 1 FROM machine_switch_value_conditions_attribute_positions AS position WHERE position."source_element_id"=219 AND position.field_kind='relation' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:confsetting.condition:value',(EXISTS(SELECT 1 FROM mame_switch_value_conditions AS value WHERE value."source_element_id"=219 AND value."value" IS '') AND EXISTS(SELECT 1 FROM machine_switch_value_conditions_attribute_positions AS position WHERE position."source_element_id"=219 AND position.field_kind='value' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:port:tag',(EXISTS(SELECT 1 FROM mame_ports AS value WHERE value."source_element_id"=220 AND value."tag" IS '') AND EXISTS(SELECT 1 FROM mame_machine_ports_attribute_positions AS position WHERE position."source_element_id"=220 AND position.field_kind='tag' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:analog:mask',(EXISTS(SELECT 1 FROM mame_analogs AS value WHERE value."source_element_id"=221 AND value."mask" IS '') AND EXISTS(SELECT 1 FROM mame_machine_analogs_attribute_positions AS position WHERE position."source_element_id"=221 AND position.field_kind='mask' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:adjuster:name',(EXISTS(SELECT 1 FROM mame_adjusters AS value WHERE value."source_element_id"=222 AND value."name" IS '') AND EXISTS(SELECT 1 FROM mame_machine_adjusters_attribute_positions AS position WHERE position."source_element_id"=222 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:adjuster:default',(EXISTS(SELECT 1 FROM mame_adjusters AS value WHERE value."source_element_id"=222 AND value."default" IS 'source CDATA default') AND EXISTS(SELECT 1 FROM mame_machine_adjusters_attribute_positions AS position WHERE position."source_element_id"=222 AND position.field_kind='default' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:adjuster.condition:tag',(EXISTS(SELECT 1 FROM mame_adjuster_conditions AS value WHERE value."source_element_id"=223 AND value."tag" IS '') AND EXISTS(SELECT 1 FROM mame_machine_adjuster_conditions_attribute_positions AS position WHERE position."source_element_id"=223 AND position.field_kind='tag' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:adjuster.condition:mask',(EXISTS(SELECT 1 FROM mame_adjuster_conditions AS value WHERE value."source_element_id"=223 AND value."mask" IS '') AND EXISTS(SELECT 1 FROM mame_machine_adjuster_conditions_attribute_positions AS position WHERE position."source_element_id"=223 AND position.field_kind='mask' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:adjuster.condition:relation',(EXISTS(SELECT 1 FROM mame_adjuster_conditions AS value WHERE value."source_element_id"=223 AND value."relation" IS 'eq') AND EXISTS(SELECT 1 FROM mame_machine_adjuster_conditions_attribute_positions AS position WHERE position."source_element_id"=223 AND position.field_kind='relation' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:adjuster.condition:value',(EXISTS(SELECT 1 FROM mame_adjuster_conditions AS value WHERE value."source_element_id"=223 AND value."value" IS '') AND EXISTS(SELECT 1 FROM mame_machine_adjuster_conditions_attribute_positions AS position WHERE position."source_element_id"=223 AND position.field_kind='value' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:driver:status',(EXISTS(SELECT 1 FROM mame_drivers AS value WHERE value."source_element_id"=224 AND value."status" IS 'good') AND EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions AS position WHERE position."source_element_id"=224 AND position.field_kind='status' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:driver:emulation',(EXISTS(SELECT 1 FROM mame_drivers AS value WHERE value."source_element_id"=224 AND value."emulation" IS 'good') AND EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions AS position WHERE position."source_element_id"=224 AND position.field_kind='emulation' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:driver:cocktail',(EXISTS(SELECT 1 FROM mame_drivers AS value WHERE value."source_element_id"=224 AND value."cocktail" IS 'good') AND EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions AS position WHERE position."source_element_id"=224 AND position.field_kind='cocktail' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:driver:savestate',(EXISTS(SELECT 1 FROM mame_drivers AS value WHERE value."source_element_id"=224 AND value."savestate" IS 'supported') AND EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions AS position WHERE position."source_element_id"=224 AND position.field_kind='savestate' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:driver:requiresartwork',(EXISTS(SELECT 1 FROM mame_drivers AS value WHERE value."source_element_id"=224 AND value."requiresartwork" IS 0 AND value."requiresartwork_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions AS position WHERE position."source_element_id"=224 AND position.field_kind='requiresartwork' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:driver:unofficial',(EXISTS(SELECT 1 FROM mame_drivers AS value WHERE value."source_element_id"=224 AND value."unofficial" IS 0 AND value."unofficial_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions AS position WHERE position."source_element_id"=224 AND position.field_kind='unofficial' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:driver:nosoundhardware',(EXISTS(SELECT 1 FROM mame_drivers AS value WHERE value."source_element_id"=224 AND value."nosoundhardware" IS 0 AND value."nosoundhardware_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions AS position WHERE position."source_element_id"=224 AND position.field_kind='nosoundhardware' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:driver:incomplete',(EXISTS(SELECT 1 FROM mame_drivers AS value WHERE value."source_element_id"=224 AND value."incomplete" IS 0 AND value."incomplete_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions AS position WHERE position."source_element_id"=224 AND position.field_kind='incomplete' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:feature:type',(EXISTS(SELECT 1 FROM mame_features AS value WHERE value."source_element_id"=225 AND value."type" IS 'protection') AND EXISTS(SELECT 1 FROM mame_machine_features_attribute_positions AS position WHERE position."source_element_id"=225 AND position.field_kind='type' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:feature:status',(EXISTS(SELECT 1 FROM mame_features AS value WHERE value."source_element_id"=225 AND value."status" IS 'unemulated') AND EXISTS(SELECT 1 FROM mame_machine_features_attribute_positions AS position WHERE position."source_element_id"=225 AND position.field_kind='status' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:feature:overall',(EXISTS(SELECT 1 FROM mame_features AS value WHERE value."source_element_id"=225 AND value."overall" IS 'unemulated') AND EXISTS(SELECT 1 FROM mame_machine_features_attribute_positions AS position WHERE position."source_element_id"=225 AND position.field_kind='overall' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:device:type',(EXISTS(SELECT 1 FROM mame_devices AS value WHERE value."source_element_id"=226 AND value."type" IS '') AND EXISTS(SELECT 1 FROM mame_machine_devices_attribute_positions AS position WHERE position."source_element_id"=226 AND position.field_kind='type' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:device:tag',(EXISTS(SELECT 1 FROM mame_devices AS value WHERE value."source_element_id"=226 AND value."tag" IS '') AND EXISTS(SELECT 1 FROM mame_machine_devices_attribute_positions AS position WHERE position."source_element_id"=226 AND position.field_kind='tag' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:device:fixed_image',(EXISTS(SELECT 1 FROM mame_devices AS value WHERE value."source_element_id"=226 AND value."fixed_image" IS '') AND EXISTS(SELECT 1 FROM mame_machine_devices_attribute_positions AS position WHERE position."source_element_id"=226 AND position.field_kind='fixed_image' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:device:mandatory',(EXISTS(SELECT 1 FROM mame_devices AS value WHERE value."source_element_id"=226 AND value."mandatory" IS '') AND EXISTS(SELECT 1 FROM mame_machine_devices_attribute_positions AS position WHERE position."source_element_id"=226 AND position.field_kind='mandatory' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:device:interface',(EXISTS(SELECT 1 FROM mame_devices AS value WHERE value."source_element_id"=226 AND value."interface" IS '') AND EXISTS(SELECT 1 FROM mame_machine_devices_attribute_positions AS position WHERE position."source_element_id"=226 AND position.field_kind='interface' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:instance:name',(EXISTS(SELECT 1 FROM mame_device_instances AS value WHERE value."source_element_id"=227 AND value."name" IS '') AND EXISTS(SELECT 1 FROM mame_machine_device_instances_attribute_positions AS position WHERE position."source_element_id"=227 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:instance:briefname',(EXISTS(SELECT 1 FROM mame_device_instances AS value WHERE value."source_element_id"=227 AND value."briefname" IS '') AND EXISTS(SELECT 1 FROM mame_machine_device_instances_attribute_positions AS position WHERE position."source_element_id"=227 AND position.field_kind='briefname' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:extension:name',(EXISTS(SELECT 1 FROM mame_device_extensions AS value WHERE value."source_element_id"=228 AND value."name" IS '') AND EXISTS(SELECT 1 FROM mame_machine_device_extensions_attribute_positions AS position WHERE position."source_element_id"=228 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:slot:name',(EXISTS(SELECT 1 FROM mame_slots AS value WHERE value."source_element_id"=229 AND value."name" IS '') AND EXISTS(SELECT 1 FROM mame_machine_slots_attribute_positions AS position WHERE position."source_element_id"=229 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:slotoption:name',(EXISTS(SELECT 1 FROM mame_slot_options AS value WHERE value."source_element_id"=230 AND value."name" IS '') AND EXISTS(SELECT 1 FROM mame_machine_slot_options_attribute_positions AS position WHERE position."source_element_id"=230 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:slotoption:devname',(EXISTS(SELECT 1 FROM mame_slot_options AS value WHERE value."source_element_id"=230 AND value."devname" IS '') AND EXISTS(SELECT 1 FROM mame_machine_slot_options_attribute_positions AS position WHERE position."source_element_id"=230 AND position.field_kind='devname' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:slotoption:default',(EXISTS(SELECT 1 FROM mame_slot_options AS value WHERE value."source_element_id"=230 AND value."default" IS 0 AND value."default_specified" IS 1) AND EXISTS(SELECT 1 FROM mame_machine_slot_options_attribute_positions AS position WHERE position."source_element_id"=230 AND position.field_kind='default' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:softwarelist:tag',(EXISTS(SELECT 1 FROM mame_softwarelist_references AS value WHERE value."source_element_id"=231 AND value."tag" IS '') AND EXISTS(SELECT 1 FROM mame_machine_software_lists_attribute_positions AS position WHERE position."source_element_id"=231 AND position.field_kind='tag' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:softwarelist:name',(EXISTS(SELECT 1 FROM mame_softwarelist_references AS value WHERE value."source_element_id"=231 AND value."name" IS '') AND EXISTS(SELECT 1 FROM mame_machine_software_lists_attribute_positions AS position WHERE position."source_element_id"=231 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:softwarelist:status',(EXISTS(SELECT 1 FROM mame_softwarelist_references AS value WHERE value."source_element_id"=231 AND value."status" IS 'original') AND EXISTS(SELECT 1 FROM mame_machine_software_lists_attribute_positions AS position WHERE position."source_element_id"=231 AND position.field_kind='status' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:softwarelist:filter',(EXISTS(SELECT 1 FROM mame_softwarelist_references AS value WHERE value."source_element_id"=231 AND value."filter" IS '') AND EXISTS(SELECT 1 FROM mame_machine_software_lists_attribute_positions AS position WHERE position."source_element_id"=231 AND position.field_kind='filter' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:ramoption:name',(EXISTS(SELECT 1 FROM mame_ram_options AS value WHERE value."source_element_id"=232 AND value."name" IS '') AND EXISTS(SELECT 1 FROM mame_machine_ram_options_attribute_positions AS position WHERE position."source_element_id"=232 AND position.field_kind='name' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:ramoption:default',(EXISTS(SELECT 1 FROM mame_ram_options AS value WHERE value."source_element_id"=232 AND value."default" IS 'not-a-boolean') AND EXISTS(SELECT 1 FROM mame_machine_ram_options_attribute_positions AS position WHERE position."source_element_id"=232 AND position.field_kind='default' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:compat.machine:isconsumable',(EXISTS(SELECT 1 FROM mame_machine_compatibility AS value WHERE value."set_id"=100 AND value."isconsumable" IS 0) AND EXISTS(SELECT 1 FROM mame_machine_compatibility_attribute_positions AS position WHERE position."set_id"=100 AND position.field_kind='isconsumable' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:compat.rom:md5',(EXISTS(SELECT 1 FROM catalog_entry_hashes AS value JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE value.reported_hash_id=503 AND value.presence='value' AND declared_text='0123456789abcdef0123456789abcdef') AND EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions AS position WHERE position."media_entry_id"=201 AND position.field_kind='md5' AND position.field_occurrence=0 AND position.reported_hash_id=503)));
INSERT INTO mame_field_assertions VALUES ('field:compat.rom:soundonly',(EXISTS(SELECT 1 FROM mame_rom_compatibility AS value WHERE value."media_entry_id"=201 AND value."soundonly" IS 0) AND EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions AS position WHERE position."media_entry_id"=201 AND position.field_kind='soundonly' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:compat.rom:dispose',(EXISTS(SELECT 1 FROM mame_rom_compatibility AS value WHERE value."media_entry_id"=201 AND value."dispose" IS 0) AND EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions AS position WHERE position."media_entry_id"=201 AND position.field_kind='dispose' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:compat.rom:loadflag',(EXISTS(SELECT 1 FROM mame_rom_compatibility AS value WHERE value."media_entry_id"=201 AND value."loadflag" IS '') AND EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions AS position WHERE position."media_entry_id"=201 AND position.field_kind='loadflag' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:compat.rom:value',(EXISTS(SELECT 1 FROM mame_rom_compatibility AS value WHERE value."media_entry_id"=201 AND value."value" IS '') AND EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions AS position WHERE position."media_entry_id"=201 AND position.field_kind='value' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:compat.rom:inverted',(EXISTS(SELECT 1 FROM mame_rom_compatibility AS value WHERE value."media_entry_id"=201 AND value."inverted" IS 0) AND EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions AS position WHERE position."media_entry_id"=201 AND position.field_kind='inverted' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:compat.rom:ovha',(EXISTS(SELECT 1 FROM mame_rom_compatibility AS value WHERE value."media_entry_id"=201 AND value."ovha" IS '') AND EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions AS position WHERE position."media_entry_id"=201 AND position.field_kind='ovha' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:compat.rom:nothread',(EXISTS(SELECT 1 FROM mame_rom_compatibility AS value WHERE value."media_entry_id"=201 AND value."nothread" IS 0) AND EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions AS position WHERE position."media_entry_id"=201 AND position.field_kind='nothread' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:compat.disk:writeable',(EXISTS(SELECT 1 FROM mame_disk_compatibility AS value WHERE value."media_entry_id"=202 AND value."writeable" IS 0) AND EXISTS(SELECT 1 FROM mame_disk_compatibility_attribute_positions AS position WHERE position."media_entry_id"=202 AND position.field_kind='writeable' AND position.field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('field:description:description',(EXISTS(SELECT 1 FROM mame_machine_text_elements AS value WHERE value.source_element_id=233 AND value."text_value" IS '' AND value.field_kind='description')));
INSERT INTO mame_field_assertions VALUES ('field:year:year',(EXISTS(SELECT 1 FROM mame_machine_text_elements AS value WHERE value.source_element_id=234 AND value."text_value" IS '' AND value.field_kind='year')));
INSERT INTO mame_field_assertions VALUES ('field:manufacturer:manufacturer',(EXISTS(SELECT 1 FROM mame_machine_text_elements AS value WHERE value.source_element_id=235 AND value."text_value" IS '' AND value.field_kind='manufacturer')));
INSERT INTO mame_field_assertions VALUES ('field:ramoption.text:text',(EXISTS(SELECT 1 FROM mame_ram_options AS value WHERE value.source_element_id=232 AND value."text" IS '128K & direct text')));

-- TEST detecting layer: typed retained-value/presence versus exact field
-- position. It is not installed by mame.sql or the main publication harness.
CREATE TEMP VIEW mame_fixture_presence_problems AS
SELECT 'mame_document_facts_attribute_positions/build' AS problem,owner."edition_id" AS owner_id FROM mame_documents AS owner WHERE (owner."build" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_document_facts_attribute_positions AS position WHERE position."edition_id"=owner."edition_id" AND position.field_kind='build' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_document_facts_attribute_positions/debug' AS problem,owner."edition_id" AS owner_id FROM mame_documents AS owner WHERE (owner."debug_specified"=1) <> (EXISTS(SELECT 1 FROM mame_document_facts_attribute_positions AS position WHERE position."edition_id"=owner."edition_id" AND position.field_kind='debug' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_document_facts_attribute_positions/mameconfig' AS problem,owner."edition_id" AS owner_id FROM mame_documents AS owner WHERE (owner."mameconfig" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_document_facts_attribute_positions AS position WHERE position."edition_id"=owner."edition_id" AND position.field_kind='mameconfig' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machines_attribute_positions/name' AS problem,owner."set_id" AS owner_id FROM mame_machines AS owner WHERE (EXISTS(SELECT 1 FROM catalog_sets AS value WHERE value."set_id"=owner."set_id" AND value."set_name" IS NOT NULL)) <> (EXISTS(SELECT 1 FROM mame_machines_attribute_positions AS position WHERE position."set_id"=owner."set_id" AND position.field_kind='name' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machines_attribute_positions/sourcefile' AS problem,owner."set_id" AS owner_id FROM mame_machines AS owner WHERE (owner."sourcefile" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machines_attribute_positions AS position WHERE position."set_id"=owner."set_id" AND position.field_kind='sourcefile' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machines_attribute_positions/isbios' AS problem,owner."set_id" AS owner_id FROM mame_machines AS owner WHERE (owner."isbios_specified"=1) <> (EXISTS(SELECT 1 FROM mame_machines_attribute_positions AS position WHERE position."set_id"=owner."set_id" AND position.field_kind='isbios' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machines_attribute_positions/isdevice' AS problem,owner."set_id" AS owner_id FROM mame_machines AS owner WHERE (owner."isdevice_specified"=1) <> (EXISTS(SELECT 1 FROM mame_machines_attribute_positions AS position WHERE position."set_id"=owner."set_id" AND position.field_kind='isdevice' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machines_attribute_positions/ismechanical' AS problem,owner."set_id" AS owner_id FROM mame_machines AS owner WHERE (owner."ismechanical_specified"=1) <> (EXISTS(SELECT 1 FROM mame_machines_attribute_positions AS position WHERE position."set_id"=owner."set_id" AND position.field_kind='ismechanical' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machines_attribute_positions/runnable' AS problem,owner."set_id" AS owner_id FROM mame_machines AS owner WHERE (owner."runnable_specified"=1) <> (EXISTS(SELECT 1 FROM mame_machines_attribute_positions AS position WHERE position."set_id"=owner."set_id" AND position.field_kind='runnable' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machines_attribute_positions/cloneof' AS problem,owner."set_id" AS owner_id FROM mame_machines AS owner WHERE (EXISTS(SELECT 1 FROM mame_machine_links AS value WHERE value.machine_id=owner."set_id" AND value.link_kind='cloneof')) <> (EXISTS(SELECT 1 FROM mame_machines_attribute_positions AS position WHERE position."set_id"=owner."set_id" AND position.field_kind='cloneof' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machines_attribute_positions/romof' AS problem,owner."set_id" AS owner_id FROM mame_machines AS owner WHERE (EXISTS(SELECT 1 FROM mame_machine_links AS value WHERE value.machine_id=owner."set_id" AND value.link_kind='romof')) <> (EXISTS(SELECT 1 FROM mame_machines_attribute_positions AS position WHERE position."set_id"=owner."set_id" AND position.field_kind='romof' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machines_attribute_positions/sampleof' AS problem,owner."set_id" AS owner_id FROM mame_machines AS owner WHERE (EXISTS(SELECT 1 FROM mame_machine_links AS value WHERE value.machine_id=owner."set_id" AND value.link_kind='sampleof')) <> (EXISTS(SELECT 1 FROM mame_machines_attribute_positions AS position WHERE position."set_id"=owner."set_id" AND position.field_kind='sampleof' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_bios_sets_attribute_positions/name' AS problem,owner."source_element_id" AS owner_id FROM mame_bios_sets AS owner WHERE (owner."name" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_bios_sets_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='name' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_bios_sets_attribute_positions/description' AS problem,owner."source_element_id" AS owner_id FROM mame_bios_sets AS owner WHERE (owner."description" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_bios_sets_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='description' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_bios_sets_attribute_positions/default' AS problem,owner."source_element_id" AS owner_id FROM mame_bios_sets AS owner WHERE (owner."default_specified"=1) <> (EXISTS(SELECT 1 FROM mame_bios_sets_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='default' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_rom_claims_attribute_positions/name' AS problem,owner."media_entry_id" AS owner_id FROM mame_roms AS owner WHERE (owner."name" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='name' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_rom_claims_attribute_positions/bios' AS problem,owner."media_entry_id" AS owner_id FROM mame_roms AS owner WHERE (owner."bios" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='bios' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_rom_claims_attribute_positions/size' AS problem,owner."media_entry_id" AS owner_id FROM mame_roms AS owner WHERE (owner."size_text" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='size' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_rom_claims_attribute_positions/crc' AS problem,owner."media_entry_id" AS owner_id FROM mame_roms AS owner WHERE (EXISTS(SELECT 1 FROM catalog_entry_hashes AS value WHERE value.media_entry_id=owner."media_entry_id" AND value.source_hash_field='crc' AND value.field_occurrence=0)) <> (EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='crc' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_rom_claims_attribute_positions/sha1' AS problem,owner."media_entry_id" AS owner_id FROM mame_roms AS owner WHERE (EXISTS(SELECT 1 FROM catalog_entry_hashes AS value WHERE value.media_entry_id=owner."media_entry_id" AND value.source_hash_field='sha1' AND value.field_occurrence=0)) <> (EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='sha1' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_rom_claims_attribute_positions/merge' AS problem,owner."media_entry_id" AS owner_id FROM mame_roms AS owner WHERE (EXISTS(SELECT 1 FROM mame_rom_merges AS value WHERE value."media_entry_id"=owner."media_entry_id" AND value."merge_name" IS NOT NULL)) <> (EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='merge' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_rom_claims_attribute_positions/region' AS problem,owner."media_entry_id" AS owner_id FROM mame_roms AS owner WHERE (owner."region" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='region' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_rom_claims_attribute_positions/offset' AS problem,owner."media_entry_id" AS owner_id FROM mame_roms AS owner WHERE (owner."offset" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='offset' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_rom_claims_attribute_positions/status' AS problem,owner."media_entry_id" AS owner_id FROM mame_roms AS owner WHERE (owner."status_specified"=1) <> (EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='status' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_rom_claims_attribute_positions/optional' AS problem,owner."media_entry_id" AS owner_id FROM mame_roms AS owner WHERE (owner."optional_specified"=1) <> (EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='optional' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_disk_claims_attribute_positions/name' AS problem,owner."media_entry_id" AS owner_id FROM mame_disks AS owner WHERE (owner."name" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='name' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_disk_claims_attribute_positions/sha1' AS problem,owner."media_entry_id" AS owner_id FROM mame_disks AS owner WHERE (EXISTS(SELECT 1 FROM catalog_entry_hashes AS value WHERE value.media_entry_id=owner."media_entry_id" AND value.source_hash_field='sha1' AND value.field_occurrence=0)) <> (EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='sha1' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_disk_claims_attribute_positions/merge' AS problem,owner."media_entry_id" AS owner_id FROM mame_disks AS owner WHERE (EXISTS(SELECT 1 FROM mame_disk_merges AS value WHERE value."media_entry_id"=owner."media_entry_id" AND value."merge_name" IS NOT NULL)) <> (EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='merge' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_disk_claims_attribute_positions/region' AS problem,owner."media_entry_id" AS owner_id FROM mame_disks AS owner WHERE (owner."region" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='region' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_disk_claims_attribute_positions/index' AS problem,owner."media_entry_id" AS owner_id FROM mame_disks AS owner WHERE (owner."index" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='index' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_disk_claims_attribute_positions/writable' AS problem,owner."media_entry_id" AS owner_id FROM mame_disks AS owner WHERE (owner."writable_specified"=1) <> (EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='writable' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_disk_claims_attribute_positions/status' AS problem,owner."media_entry_id" AS owner_id FROM mame_disks AS owner WHERE (owner."status_specified"=1) <> (EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='status' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_disk_claims_attribute_positions/optional' AS problem,owner."media_entry_id" AS owner_id FROM mame_disks AS owner WHERE (owner."optional_specified"=1) <> (EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='optional' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_device_references_attribute_positions/tag' AS problem,owner."source_element_id" AS owner_id FROM mame_device_references AS owner WHERE (owner."tag" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_device_references_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='tag' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_device_references_attribute_positions/name' AS problem,owner."source_element_id" AS owner_id FROM mame_device_references AS owner WHERE (owner."name" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_device_references_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='name' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_samples_attribute_positions/name' AS problem,owner."media_entry_id" AS owner_id FROM mame_samples AS owner WHERE (owner."name" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_samples_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='name' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_chips_attribute_positions/name' AS problem,owner."source_element_id" AS owner_id FROM mame_chips AS owner WHERE (owner."name" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_chips_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='name' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_chips_attribute_positions/tag' AS problem,owner."source_element_id" AS owner_id FROM mame_chips AS owner WHERE (owner."tag" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_chips_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='tag' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_chips_attribute_positions/type' AS problem,owner."source_element_id" AS owner_id FROM mame_chips AS owner WHERE (owner."type" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_chips_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='type' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_chips_attribute_positions/clock' AS problem,owner."source_element_id" AS owner_id FROM mame_chips AS owner WHERE (owner."clock" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_chips_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='clock' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_displays_attribute_positions/tag' AS problem,owner."source_element_id" AS owner_id FROM mame_displays AS owner WHERE (owner."tag" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='tag' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_displays_attribute_positions/type' AS problem,owner."source_element_id" AS owner_id FROM mame_displays AS owner WHERE (owner."type" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='type' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_displays_attribute_positions/rotate' AS problem,owner."source_element_id" AS owner_id FROM mame_displays AS owner WHERE (owner."rotate" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='rotate' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_displays_attribute_positions/flipx' AS problem,owner."source_element_id" AS owner_id FROM mame_displays AS owner WHERE (owner."flipx_specified"=1) <> (EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='flipx' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_displays_attribute_positions/width' AS problem,owner."source_element_id" AS owner_id FROM mame_displays AS owner WHERE (owner."width" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='width' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_displays_attribute_positions/height' AS problem,owner."source_element_id" AS owner_id FROM mame_displays AS owner WHERE (owner."height" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='height' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_displays_attribute_positions/refresh' AS problem,owner."source_element_id" AS owner_id FROM mame_displays AS owner WHERE (owner."refresh" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='refresh' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_displays_attribute_positions/pixclock' AS problem,owner."source_element_id" AS owner_id FROM mame_displays AS owner WHERE (owner."pixclock" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='pixclock' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_displays_attribute_positions/htotal' AS problem,owner."source_element_id" AS owner_id FROM mame_displays AS owner WHERE (owner."htotal" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='htotal' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_displays_attribute_positions/hbend' AS problem,owner."source_element_id" AS owner_id FROM mame_displays AS owner WHERE (owner."hbend" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='hbend' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_displays_attribute_positions/hbstart' AS problem,owner."source_element_id" AS owner_id FROM mame_displays AS owner WHERE (owner."hbstart" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='hbstart' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_displays_attribute_positions/vtotal' AS problem,owner."source_element_id" AS owner_id FROM mame_displays AS owner WHERE (owner."vtotal" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='vtotal' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_displays_attribute_positions/vbend' AS problem,owner."source_element_id" AS owner_id FROM mame_displays AS owner WHERE (owner."vbend" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='vbend' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_displays_attribute_positions/vbstart' AS problem,owner."source_element_id" AS owner_id FROM mame_displays AS owner WHERE (owner."vbstart" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='vbstart' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_sounds_attribute_positions/channels' AS problem,owner."source_element_id" AS owner_id FROM mame_sound AS owner WHERE (owner."channels" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_sounds_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='channels' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_inputs_attribute_positions/service' AS problem,owner."source_element_id" AS owner_id FROM mame_inputs AS owner WHERE (owner."service_specified"=1) <> (EXISTS(SELECT 1 FROM mame_machine_inputs_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='service' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_inputs_attribute_positions/tilt' AS problem,owner."source_element_id" AS owner_id FROM mame_inputs AS owner WHERE (owner."tilt_specified"=1) <> (EXISTS(SELECT 1 FROM mame_machine_inputs_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='tilt' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_inputs_attribute_positions/players' AS problem,owner."source_element_id" AS owner_id FROM mame_inputs AS owner WHERE (owner."players" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_inputs_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='players' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_inputs_attribute_positions/coins' AS problem,owner."source_element_id" AS owner_id FROM mame_inputs AS owner WHERE (owner."coins" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_inputs_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='coins' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_input_controls_attribute_positions/type' AS problem,owner."source_element_id" AS owner_id FROM mame_input_controls AS owner WHERE (owner."type" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='type' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_input_controls_attribute_positions/player' AS problem,owner."source_element_id" AS owner_id FROM mame_input_controls AS owner WHERE (owner."player" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='player' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_input_controls_attribute_positions/buttons' AS problem,owner."source_element_id" AS owner_id FROM mame_input_controls AS owner WHERE (owner."buttons" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='buttons' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_input_controls_attribute_positions/minimum' AS problem,owner."source_element_id" AS owner_id FROM mame_input_controls AS owner WHERE (owner."minimum" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='minimum' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_input_controls_attribute_positions/maximum' AS problem,owner."source_element_id" AS owner_id FROM mame_input_controls AS owner WHERE (owner."maximum" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='maximum' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_input_controls_attribute_positions/sensitivity' AS problem,owner."source_element_id" AS owner_id FROM mame_input_controls AS owner WHERE (owner."sensitivity" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='sensitivity' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_input_controls_attribute_positions/keydelta' AS problem,owner."source_element_id" AS owner_id FROM mame_input_controls AS owner WHERE (owner."keydelta" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='keydelta' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_input_controls_attribute_positions/reverse' AS problem,owner."source_element_id" AS owner_id FROM mame_input_controls AS owner WHERE (owner."reverse_specified"=1) <> (EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='reverse' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_input_controls_attribute_positions/ways' AS problem,owner."source_element_id" AS owner_id FROM mame_input_controls AS owner WHERE (owner."ways" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='ways' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_input_controls_attribute_positions/ways2' AS problem,owner."source_element_id" AS owner_id FROM mame_input_controls AS owner WHERE (owner."ways2" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='ways2' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_input_controls_attribute_positions/ways3' AS problem,owner."source_element_id" AS owner_id FROM mame_input_controls AS owner WHERE (owner."ways3" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='ways3' AND position.field_occurrence=0))
UNION ALL
SELECT 'machine_switches_attribute_positions/name' AS problem,owner."source_element_id" AS owner_id FROM mame_switches AS owner WHERE (owner."name" IS NOT NULL) <> (EXISTS(SELECT 1 FROM machine_switches_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='name' AND position.field_occurrence=0))
UNION ALL
SELECT 'machine_switches_attribute_positions/tag' AS problem,owner."source_element_id" AS owner_id FROM mame_switches AS owner WHERE (owner."tag" IS NOT NULL) <> (EXISTS(SELECT 1 FROM machine_switches_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='tag' AND position.field_occurrence=0))
UNION ALL
SELECT 'machine_switches_attribute_positions/mask' AS problem,owner."source_element_id" AS owner_id FROM mame_switches AS owner WHERE (owner."mask" IS NOT NULL) <> (EXISTS(SELECT 1 FROM machine_switches_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='mask' AND position.field_occurrence=0))
UNION ALL
SELECT 'machine_switch_locations_attribute_positions/name' AS problem,owner."source_element_id" AS owner_id FROM mame_switch_locations AS owner WHERE (owner."name" IS NOT NULL) <> (EXISTS(SELECT 1 FROM machine_switch_locations_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='name' AND position.field_occurrence=0))
UNION ALL
SELECT 'machine_switch_locations_attribute_positions/number' AS problem,owner."source_element_id" AS owner_id FROM mame_switch_locations AS owner WHERE (owner."number" IS NOT NULL) <> (EXISTS(SELECT 1 FROM machine_switch_locations_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='number' AND position.field_occurrence=0))
UNION ALL
SELECT 'machine_switch_locations_attribute_positions/inverted' AS problem,owner."source_element_id" AS owner_id FROM mame_switch_locations AS owner WHERE (owner."inverted_specified"=1) <> (EXISTS(SELECT 1 FROM machine_switch_locations_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='inverted' AND position.field_occurrence=0))
UNION ALL
SELECT 'machine_switch_values_attribute_positions/name' AS problem,owner."source_element_id" AS owner_id FROM mame_switch_values AS owner WHERE (owner."name" IS NOT NULL) <> (EXISTS(SELECT 1 FROM machine_switch_values_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='name' AND position.field_occurrence=0))
UNION ALL
SELECT 'machine_switch_values_attribute_positions/value' AS problem,owner."source_element_id" AS owner_id FROM mame_switch_values AS owner WHERE (owner."value" IS NOT NULL) <> (EXISTS(SELECT 1 FROM machine_switch_values_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='value' AND position.field_occurrence=0))
UNION ALL
SELECT 'machine_switch_values_attribute_positions/default' AS problem,owner."source_element_id" AS owner_id FROM mame_switch_values AS owner WHERE (owner."default_specified"=1) <> (EXISTS(SELECT 1 FROM machine_switch_values_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='default' AND position.field_occurrence=0))
UNION ALL
SELECT 'machine_switch_conditions_attribute_positions/tag' AS problem,owner."source_element_id" AS owner_id FROM mame_switch_conditions AS owner WHERE (owner."tag" IS NOT NULL) <> (EXISTS(SELECT 1 FROM machine_switch_conditions_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='tag' AND position.field_occurrence=0))
UNION ALL
SELECT 'machine_switch_conditions_attribute_positions/mask' AS problem,owner."source_element_id" AS owner_id FROM mame_switch_conditions AS owner WHERE (owner."mask" IS NOT NULL) <> (EXISTS(SELECT 1 FROM machine_switch_conditions_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='mask' AND position.field_occurrence=0))
UNION ALL
SELECT 'machine_switch_conditions_attribute_positions/relation' AS problem,owner."source_element_id" AS owner_id FROM mame_switch_conditions AS owner WHERE (owner."relation" IS NOT NULL) <> (EXISTS(SELECT 1 FROM machine_switch_conditions_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='relation' AND position.field_occurrence=0))
UNION ALL
SELECT 'machine_switch_conditions_attribute_positions/value' AS problem,owner."source_element_id" AS owner_id FROM mame_switch_conditions AS owner WHERE (owner."value" IS NOT NULL) <> (EXISTS(SELECT 1 FROM machine_switch_conditions_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='value' AND position.field_occurrence=0))
UNION ALL
SELECT 'machine_switch_value_conditions_attribute_positions/tag' AS problem,owner."source_element_id" AS owner_id FROM mame_switch_value_conditions AS owner WHERE (owner."tag" IS NOT NULL) <> (EXISTS(SELECT 1 FROM machine_switch_value_conditions_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='tag' AND position.field_occurrence=0))
UNION ALL
SELECT 'machine_switch_value_conditions_attribute_positions/mask' AS problem,owner."source_element_id" AS owner_id FROM mame_switch_value_conditions AS owner WHERE (owner."mask" IS NOT NULL) <> (EXISTS(SELECT 1 FROM machine_switch_value_conditions_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='mask' AND position.field_occurrence=0))
UNION ALL
SELECT 'machine_switch_value_conditions_attribute_positions/relation' AS problem,owner."source_element_id" AS owner_id FROM mame_switch_value_conditions AS owner WHERE (owner."relation" IS NOT NULL) <> (EXISTS(SELECT 1 FROM machine_switch_value_conditions_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='relation' AND position.field_occurrence=0))
UNION ALL
SELECT 'machine_switch_value_conditions_attribute_positions/value' AS problem,owner."source_element_id" AS owner_id FROM mame_switch_value_conditions AS owner WHERE (owner."value" IS NOT NULL) <> (EXISTS(SELECT 1 FROM machine_switch_value_conditions_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='value' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_ports_attribute_positions/tag' AS problem,owner."source_element_id" AS owner_id FROM mame_ports AS owner WHERE (owner."tag" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_ports_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='tag' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_analogs_attribute_positions/mask' AS problem,owner."source_element_id" AS owner_id FROM mame_analogs AS owner WHERE (owner."mask" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_analogs_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='mask' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_adjusters_attribute_positions/name' AS problem,owner."source_element_id" AS owner_id FROM mame_adjusters AS owner WHERE (owner."name" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_adjusters_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='name' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_adjusters_attribute_positions/default' AS problem,owner."source_element_id" AS owner_id FROM mame_adjusters AS owner WHERE (owner."default" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_adjusters_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='default' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_adjuster_conditions_attribute_positions/tag' AS problem,owner."source_element_id" AS owner_id FROM mame_adjuster_conditions AS owner WHERE (owner."tag" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_adjuster_conditions_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='tag' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_adjuster_conditions_attribute_positions/mask' AS problem,owner."source_element_id" AS owner_id FROM mame_adjuster_conditions AS owner WHERE (owner."mask" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_adjuster_conditions_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='mask' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_adjuster_conditions_attribute_positions/relation' AS problem,owner."source_element_id" AS owner_id FROM mame_adjuster_conditions AS owner WHERE (owner."relation" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_adjuster_conditions_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='relation' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_adjuster_conditions_attribute_positions/value' AS problem,owner."source_element_id" AS owner_id FROM mame_adjuster_conditions AS owner WHERE (owner."value" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_adjuster_conditions_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='value' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_drivers_attribute_positions/status' AS problem,owner."source_element_id" AS owner_id FROM mame_drivers AS owner WHERE (owner."status" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='status' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_drivers_attribute_positions/emulation' AS problem,owner."source_element_id" AS owner_id FROM mame_drivers AS owner WHERE (owner."emulation" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='emulation' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_drivers_attribute_positions/cocktail' AS problem,owner."source_element_id" AS owner_id FROM mame_drivers AS owner WHERE (owner."cocktail" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='cocktail' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_drivers_attribute_positions/savestate' AS problem,owner."source_element_id" AS owner_id FROM mame_drivers AS owner WHERE (owner."savestate" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='savestate' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_drivers_attribute_positions/requiresartwork' AS problem,owner."source_element_id" AS owner_id FROM mame_drivers AS owner WHERE (owner."requiresartwork_specified"=1) <> (EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='requiresartwork' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_drivers_attribute_positions/unofficial' AS problem,owner."source_element_id" AS owner_id FROM mame_drivers AS owner WHERE (owner."unofficial_specified"=1) <> (EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='unofficial' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_drivers_attribute_positions/nosoundhardware' AS problem,owner."source_element_id" AS owner_id FROM mame_drivers AS owner WHERE (owner."nosoundhardware_specified"=1) <> (EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='nosoundhardware' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_drivers_attribute_positions/incomplete' AS problem,owner."source_element_id" AS owner_id FROM mame_drivers AS owner WHERE (owner."incomplete_specified"=1) <> (EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='incomplete' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_features_attribute_positions/type' AS problem,owner."source_element_id" AS owner_id FROM mame_features AS owner WHERE (owner."type" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_features_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='type' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_features_attribute_positions/status' AS problem,owner."source_element_id" AS owner_id FROM mame_features AS owner WHERE (owner."status" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_features_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='status' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_features_attribute_positions/overall' AS problem,owner."source_element_id" AS owner_id FROM mame_features AS owner WHERE (owner."overall" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_features_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='overall' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_devices_attribute_positions/type' AS problem,owner."source_element_id" AS owner_id FROM mame_devices AS owner WHERE (owner."type" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_devices_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='type' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_devices_attribute_positions/tag' AS problem,owner."source_element_id" AS owner_id FROM mame_devices AS owner WHERE (owner."tag" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_devices_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='tag' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_devices_attribute_positions/fixed_image' AS problem,owner."source_element_id" AS owner_id FROM mame_devices AS owner WHERE (owner."fixed_image" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_devices_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='fixed_image' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_devices_attribute_positions/mandatory' AS problem,owner."source_element_id" AS owner_id FROM mame_devices AS owner WHERE (owner."mandatory" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_devices_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='mandatory' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_devices_attribute_positions/interface' AS problem,owner."source_element_id" AS owner_id FROM mame_devices AS owner WHERE (owner."interface" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_devices_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='interface' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_device_instances_attribute_positions/name' AS problem,owner."source_element_id" AS owner_id FROM mame_device_instances AS owner WHERE (owner."name" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_device_instances_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='name' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_device_instances_attribute_positions/briefname' AS problem,owner."source_element_id" AS owner_id FROM mame_device_instances AS owner WHERE (owner."briefname" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_device_instances_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='briefname' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_device_extensions_attribute_positions/name' AS problem,owner."source_element_id" AS owner_id FROM mame_device_extensions AS owner WHERE (owner."name" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_device_extensions_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='name' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_slots_attribute_positions/name' AS problem,owner."source_element_id" AS owner_id FROM mame_slots AS owner WHERE (owner."name" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_slots_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='name' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_slot_options_attribute_positions/name' AS problem,owner."source_element_id" AS owner_id FROM mame_slot_options AS owner WHERE (owner."name" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_slot_options_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='name' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_slot_options_attribute_positions/devname' AS problem,owner."source_element_id" AS owner_id FROM mame_slot_options AS owner WHERE (owner."devname" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_slot_options_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='devname' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_slot_options_attribute_positions/default' AS problem,owner."source_element_id" AS owner_id FROM mame_slot_options AS owner WHERE (owner."default_specified"=1) <> (EXISTS(SELECT 1 FROM mame_machine_slot_options_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='default' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_software_lists_attribute_positions/tag' AS problem,owner."source_element_id" AS owner_id FROM mame_softwarelist_references AS owner WHERE (owner."tag" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_software_lists_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='tag' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_software_lists_attribute_positions/name' AS problem,owner."source_element_id" AS owner_id FROM mame_softwarelist_references AS owner WHERE (owner."name" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_software_lists_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='name' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_software_lists_attribute_positions/status' AS problem,owner."source_element_id" AS owner_id FROM mame_softwarelist_references AS owner WHERE (owner."status" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_software_lists_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='status' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_software_lists_attribute_positions/filter' AS problem,owner."source_element_id" AS owner_id FROM mame_softwarelist_references AS owner WHERE (owner."filter" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_software_lists_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='filter' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_ram_options_attribute_positions/name' AS problem,owner."source_element_id" AS owner_id FROM mame_ram_options AS owner WHERE (owner."name" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_ram_options_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='name' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_ram_options_attribute_positions/default' AS problem,owner."source_element_id" AS owner_id FROM mame_ram_options AS owner WHERE (owner."default" IS NOT NULL) <> (EXISTS(SELECT 1 FROM mame_machine_ram_options_attribute_positions AS position WHERE position."source_element_id"=owner."source_element_id" AND position.field_kind='default' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_machine_compatibility_attribute_positions/isconsumable' AS problem,owner."set_id" AS owner_id FROM mame_machines AS owner WHERE (EXISTS(SELECT 1 FROM mame_machine_compatibility AS value WHERE value."set_id"=owner."set_id" AND value."isconsumable" IS NOT NULL)) <> (EXISTS(SELECT 1 FROM mame_machine_compatibility_attribute_positions AS position WHERE position."set_id"=owner."set_id" AND position.field_kind='isconsumable' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_rom_compatibility_attribute_positions/md5' AS problem,owner."media_entry_id" AS owner_id FROM mame_roms AS owner WHERE (EXISTS(SELECT 1 FROM catalog_entry_hashes AS value WHERE value.media_entry_id=owner."media_entry_id" AND value.source_hash_field='md5' AND value.field_occurrence=0)) <> (EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='md5' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_rom_compatibility_attribute_positions/soundonly' AS problem,owner."media_entry_id" AS owner_id FROM mame_roms AS owner WHERE (EXISTS(SELECT 1 FROM mame_rom_compatibility AS value WHERE value."media_entry_id"=owner."media_entry_id" AND value."soundonly" IS NOT NULL)) <> (EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='soundonly' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_rom_compatibility_attribute_positions/dispose' AS problem,owner."media_entry_id" AS owner_id FROM mame_roms AS owner WHERE (EXISTS(SELECT 1 FROM mame_rom_compatibility AS value WHERE value."media_entry_id"=owner."media_entry_id" AND value."dispose" IS NOT NULL)) <> (EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='dispose' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_rom_compatibility_attribute_positions/loadflag' AS problem,owner."media_entry_id" AS owner_id FROM mame_roms AS owner WHERE (EXISTS(SELECT 1 FROM mame_rom_compatibility AS value WHERE value."media_entry_id"=owner."media_entry_id" AND value."loadflag" IS NOT NULL)) <> (EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='loadflag' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_rom_compatibility_attribute_positions/value' AS problem,owner."media_entry_id" AS owner_id FROM mame_roms AS owner WHERE (EXISTS(SELECT 1 FROM mame_rom_compatibility AS value WHERE value."media_entry_id"=owner."media_entry_id" AND value."value" IS NOT NULL)) <> (EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='value' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_rom_compatibility_attribute_positions/inverted' AS problem,owner."media_entry_id" AS owner_id FROM mame_roms AS owner WHERE (EXISTS(SELECT 1 FROM mame_rom_compatibility AS value WHERE value."media_entry_id"=owner."media_entry_id" AND value."inverted" IS NOT NULL)) <> (EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='inverted' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_rom_compatibility_attribute_positions/ovha' AS problem,owner."media_entry_id" AS owner_id FROM mame_roms AS owner WHERE (EXISTS(SELECT 1 FROM mame_rom_compatibility AS value WHERE value."media_entry_id"=owner."media_entry_id" AND value."ovha" IS NOT NULL)) <> (EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='ovha' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_rom_compatibility_attribute_positions/nothread' AS problem,owner."media_entry_id" AS owner_id FROM mame_roms AS owner WHERE (EXISTS(SELECT 1 FROM mame_rom_compatibility AS value WHERE value."media_entry_id"=owner."media_entry_id" AND value."nothread" IS NOT NULL)) <> (EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='nothread' AND position.field_occurrence=0))
UNION ALL
SELECT 'mame_disk_compatibility_attribute_positions/writeable' AS problem,owner."media_entry_id" AS owner_id FROM mame_disks AS owner WHERE (EXISTS(SELECT 1 FROM mame_disk_compatibility AS value WHERE value."media_entry_id"=owner."media_entry_id" AND value."writeable" IS NOT NULL)) <> (EXISTS(SELECT 1 FROM mame_disk_compatibility_attribute_positions AS position WHERE position."media_entry_id"=owner."media_entry_id" AND position.field_kind='writeable' AND position.field_occurrence=0));
INSERT INTO mame_field_assertions VALUES ('baseline:typed_presence',(SELECT count(*)=0 FROM mame_fixture_presence_problems));

-- Required columns reject NULL; required text permits explicit empty CDATA.
UPDATE OR IGNORE mame_documents SET "mameconfig"=NULL WHERE "edition_id"=1;
INSERT INTO mame_field_assertions VALUES ('required:mame:mameconfig',(changes()=0));
UPDATE OR IGNORE catalog_sets SET "set_name"=NULL WHERE "set_id"=100;
INSERT INTO mame_field_assertions VALUES ('required:machine:name',(changes()=0));
UPDATE OR IGNORE mame_bios_sets SET "name"=NULL WHERE "source_element_id"=200;
INSERT INTO mame_field_assertions VALUES ('required:biosset:name',(changes()=0));
UPDATE OR IGNORE mame_bios_sets SET "description"=NULL WHERE "source_element_id"=200;
INSERT INTO mame_field_assertions VALUES ('required:biosset:description',(changes()=0));
UPDATE OR IGNORE mame_roms SET "name"=NULL WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('required:rom:name',(changes()=0));
UPDATE OR IGNORE mame_disks SET "name"=NULL WHERE "media_entry_id"=202;
INSERT INTO mame_field_assertions VALUES ('required:disk:name',(changes()=0));
UPDATE OR IGNORE mame_device_references SET "tag"=NULL WHERE "source_element_id"=203;
INSERT INTO mame_field_assertions VALUES ('required:device_ref:tag',(changes()=0));
UPDATE OR IGNORE mame_device_references SET "name"=NULL WHERE "source_element_id"=203;
INSERT INTO mame_field_assertions VALUES ('required:device_ref:name',(changes()=0));
UPDATE OR IGNORE mame_samples SET "name"=NULL WHERE "media_entry_id"=204;
INSERT INTO mame_field_assertions VALUES ('required:sample:name',(changes()=0));
UPDATE OR IGNORE mame_chips SET "name"=NULL WHERE "source_element_id"=205;
INSERT INTO mame_field_assertions VALUES ('required:chip:name',(changes()=0));
UPDATE OR IGNORE mame_chips SET "type"=NULL WHERE "source_element_id"=205;
INSERT INTO mame_field_assertions VALUES ('required:chip:type',(changes()=0));
UPDATE OR IGNORE mame_chips SET "type"='' WHERE "source_element_id"=205;
INSERT INTO mame_field_assertions VALUES ('enum:chip:type',(changes()=0));
UPDATE OR IGNORE mame_displays SET "type"=NULL WHERE "source_element_id"=206;
INSERT INTO mame_field_assertions VALUES ('required:display:type',(changes()=0));
UPDATE OR IGNORE mame_displays SET "type"='' WHERE "source_element_id"=206;
INSERT INTO mame_field_assertions VALUES ('enum:display:type',(changes()=0));
UPDATE OR IGNORE mame_displays SET "refresh"=NULL WHERE "source_element_id"=206;
INSERT INTO mame_field_assertions VALUES ('required:display:refresh',(changes()=0));
UPDATE OR IGNORE mame_sound SET "channels"=NULL WHERE "source_element_id"=207;
INSERT INTO mame_field_assertions VALUES ('required:sound:channels',(changes()=0));
UPDATE OR IGNORE mame_inputs SET "players"=NULL WHERE "source_element_id"=208;
INSERT INTO mame_field_assertions VALUES ('required:input:players',(changes()=0));
UPDATE OR IGNORE mame_input_controls SET "type"=NULL WHERE "source_element_id"=209;
INSERT INTO mame_field_assertions VALUES ('required:control:type',(changes()=0));
UPDATE OR IGNORE mame_switches SET "name"=NULL WHERE "source_element_id"=210;
INSERT INTO mame_field_assertions VALUES ('required:dipswitch:name',(changes()=0));
UPDATE OR IGNORE mame_switches SET "tag"=NULL WHERE "source_element_id"=210;
INSERT INTO mame_field_assertions VALUES ('required:dipswitch:tag',(changes()=0));
UPDATE OR IGNORE mame_switches SET "mask"=NULL WHERE "source_element_id"=210;
INSERT INTO mame_field_assertions VALUES ('required:dipswitch:mask',(changes()=0));
UPDATE OR IGNORE mame_switch_locations SET "name"=NULL WHERE "source_element_id"=211;
INSERT INTO mame_field_assertions VALUES ('required:diplocation:name',(changes()=0));
UPDATE OR IGNORE mame_switch_locations SET "number"=NULL WHERE "source_element_id"=211;
INSERT INTO mame_field_assertions VALUES ('required:diplocation:number',(changes()=0));
UPDATE OR IGNORE mame_switch_values SET "name"=NULL WHERE "source_element_id"=212;
INSERT INTO mame_field_assertions VALUES ('required:dipvalue:name',(changes()=0));
UPDATE OR IGNORE mame_switch_values SET "value"=NULL WHERE "source_element_id"=212;
INSERT INTO mame_field_assertions VALUES ('required:dipvalue:value',(changes()=0));
UPDATE OR IGNORE mame_switches SET "name"=NULL WHERE "source_element_id"=213;
INSERT INTO mame_field_assertions VALUES ('required:configuration:name',(changes()=0));
UPDATE OR IGNORE mame_switches SET "tag"=NULL WHERE "source_element_id"=213;
INSERT INTO mame_field_assertions VALUES ('required:configuration:tag',(changes()=0));
UPDATE OR IGNORE mame_switches SET "mask"=NULL WHERE "source_element_id"=213;
INSERT INTO mame_field_assertions VALUES ('required:configuration:mask',(changes()=0));
UPDATE OR IGNORE mame_switch_locations SET "name"=NULL WHERE "source_element_id"=214;
INSERT INTO mame_field_assertions VALUES ('required:conflocation:name',(changes()=0));
UPDATE OR IGNORE mame_switch_locations SET "number"=NULL WHERE "source_element_id"=214;
INSERT INTO mame_field_assertions VALUES ('required:conflocation:number',(changes()=0));
UPDATE OR IGNORE mame_switch_values SET "name"=NULL WHERE "source_element_id"=215;
INSERT INTO mame_field_assertions VALUES ('required:confsetting:name',(changes()=0));
UPDATE OR IGNORE mame_switch_values SET "value"=NULL WHERE "source_element_id"=215;
INSERT INTO mame_field_assertions VALUES ('required:confsetting:value',(changes()=0));
UPDATE OR IGNORE mame_switch_conditions SET "tag"=NULL WHERE "source_element_id"=216;
INSERT INTO mame_field_assertions VALUES ('required:dipswitch.condition:tag',(changes()=0));
UPDATE OR IGNORE mame_switch_conditions SET "mask"=NULL WHERE "source_element_id"=216;
INSERT INTO mame_field_assertions VALUES ('required:dipswitch.condition:mask',(changes()=0));
UPDATE OR IGNORE mame_switch_conditions SET "relation"=NULL WHERE "source_element_id"=216;
INSERT INTO mame_field_assertions VALUES ('required:dipswitch.condition:relation',(changes()=0));
UPDATE OR IGNORE mame_switch_conditions SET "relation"='' WHERE "source_element_id"=216;
INSERT INTO mame_field_assertions VALUES ('enum:dipswitch.condition:relation',(changes()=0));
UPDATE OR IGNORE mame_switch_conditions SET "value"=NULL WHERE "source_element_id"=216;
INSERT INTO mame_field_assertions VALUES ('required:dipswitch.condition:value',(changes()=0));
UPDATE OR IGNORE mame_switch_conditions SET "tag"=NULL WHERE "source_element_id"=217;
INSERT INTO mame_field_assertions VALUES ('required:configuration.condition:tag',(changes()=0));
UPDATE OR IGNORE mame_switch_conditions SET "mask"=NULL WHERE "source_element_id"=217;
INSERT INTO mame_field_assertions VALUES ('required:configuration.condition:mask',(changes()=0));
UPDATE OR IGNORE mame_switch_conditions SET "relation"=NULL WHERE "source_element_id"=217;
INSERT INTO mame_field_assertions VALUES ('required:configuration.condition:relation',(changes()=0));
UPDATE OR IGNORE mame_switch_conditions SET "relation"='' WHERE "source_element_id"=217;
INSERT INTO mame_field_assertions VALUES ('enum:configuration.condition:relation',(changes()=0));
UPDATE OR IGNORE mame_switch_conditions SET "value"=NULL WHERE "source_element_id"=217;
INSERT INTO mame_field_assertions VALUES ('required:configuration.condition:value',(changes()=0));
UPDATE OR IGNORE mame_switch_value_conditions SET "tag"=NULL WHERE "source_element_id"=218;
INSERT INTO mame_field_assertions VALUES ('required:dipvalue.condition:tag',(changes()=0));
UPDATE OR IGNORE mame_switch_value_conditions SET "mask"=NULL WHERE "source_element_id"=218;
INSERT INTO mame_field_assertions VALUES ('required:dipvalue.condition:mask',(changes()=0));
UPDATE OR IGNORE mame_switch_value_conditions SET "relation"=NULL WHERE "source_element_id"=218;
INSERT INTO mame_field_assertions VALUES ('required:dipvalue.condition:relation',(changes()=0));
UPDATE OR IGNORE mame_switch_value_conditions SET "relation"='' WHERE "source_element_id"=218;
INSERT INTO mame_field_assertions VALUES ('enum:dipvalue.condition:relation',(changes()=0));
UPDATE OR IGNORE mame_switch_value_conditions SET "value"=NULL WHERE "source_element_id"=218;
INSERT INTO mame_field_assertions VALUES ('required:dipvalue.condition:value',(changes()=0));
UPDATE OR IGNORE mame_switch_value_conditions SET "tag"=NULL WHERE "source_element_id"=219;
INSERT INTO mame_field_assertions VALUES ('required:confsetting.condition:tag',(changes()=0));
UPDATE OR IGNORE mame_switch_value_conditions SET "mask"=NULL WHERE "source_element_id"=219;
INSERT INTO mame_field_assertions VALUES ('required:confsetting.condition:mask',(changes()=0));
UPDATE OR IGNORE mame_switch_value_conditions SET "relation"=NULL WHERE "source_element_id"=219;
INSERT INTO mame_field_assertions VALUES ('required:confsetting.condition:relation',(changes()=0));
UPDATE OR IGNORE mame_switch_value_conditions SET "relation"='' WHERE "source_element_id"=219;
INSERT INTO mame_field_assertions VALUES ('enum:confsetting.condition:relation',(changes()=0));
UPDATE OR IGNORE mame_switch_value_conditions SET "value"=NULL WHERE "source_element_id"=219;
INSERT INTO mame_field_assertions VALUES ('required:confsetting.condition:value',(changes()=0));
UPDATE OR IGNORE mame_ports SET "tag"=NULL WHERE "source_element_id"=220;
INSERT INTO mame_field_assertions VALUES ('required:port:tag',(changes()=0));
UPDATE OR IGNORE mame_analogs SET "mask"=NULL WHERE "source_element_id"=221;
INSERT INTO mame_field_assertions VALUES ('required:analog:mask',(changes()=0));
UPDATE OR IGNORE mame_adjusters SET "name"=NULL WHERE "source_element_id"=222;
INSERT INTO mame_field_assertions VALUES ('required:adjuster:name',(changes()=0));
UPDATE OR IGNORE mame_adjusters SET "default"=NULL WHERE "source_element_id"=222;
INSERT INTO mame_field_assertions VALUES ('required:adjuster:default',(changes()=0));
UPDATE OR IGNORE mame_adjuster_conditions SET "tag"=NULL WHERE "source_element_id"=223;
INSERT INTO mame_field_assertions VALUES ('required:adjuster.condition:tag',(changes()=0));
UPDATE OR IGNORE mame_adjuster_conditions SET "mask"=NULL WHERE "source_element_id"=223;
INSERT INTO mame_field_assertions VALUES ('required:adjuster.condition:mask',(changes()=0));
UPDATE OR IGNORE mame_adjuster_conditions SET "relation"=NULL WHERE "source_element_id"=223;
INSERT INTO mame_field_assertions VALUES ('required:adjuster.condition:relation',(changes()=0));
UPDATE OR IGNORE mame_adjuster_conditions SET "relation"='' WHERE "source_element_id"=223;
INSERT INTO mame_field_assertions VALUES ('enum:adjuster.condition:relation',(changes()=0));
UPDATE OR IGNORE mame_adjuster_conditions SET "value"=NULL WHERE "source_element_id"=223;
INSERT INTO mame_field_assertions VALUES ('required:adjuster.condition:value',(changes()=0));
UPDATE OR IGNORE mame_drivers SET "status"=NULL WHERE "source_element_id"=224;
INSERT INTO mame_field_assertions VALUES ('required:driver:status',(changes()=0));
UPDATE OR IGNORE mame_drivers SET "status"='' WHERE "source_element_id"=224;
INSERT INTO mame_field_assertions VALUES ('enum:driver:status',(changes()=0));
UPDATE OR IGNORE mame_drivers SET "emulation"=NULL WHERE "source_element_id"=224;
INSERT INTO mame_field_assertions VALUES ('required:driver:emulation',(changes()=0));
UPDATE OR IGNORE mame_drivers SET "emulation"='' WHERE "source_element_id"=224;
INSERT INTO mame_field_assertions VALUES ('enum:driver:emulation',(changes()=0));
UPDATE OR IGNORE mame_drivers SET "savestate"=NULL WHERE "source_element_id"=224;
INSERT INTO mame_field_assertions VALUES ('required:driver:savestate',(changes()=0));
UPDATE OR IGNORE mame_drivers SET "savestate"='' WHERE "source_element_id"=224;
INSERT INTO mame_field_assertions VALUES ('enum:driver:savestate',(changes()=0));
UPDATE OR IGNORE mame_features SET "type"=NULL WHERE "source_element_id"=225;
INSERT INTO mame_field_assertions VALUES ('required:feature:type',(changes()=0));
UPDATE OR IGNORE mame_features SET "type"='' WHERE "source_element_id"=225;
INSERT INTO mame_field_assertions VALUES ('enum:feature:type',(changes()=0));
UPDATE OR IGNORE mame_devices SET "type"=NULL WHERE "source_element_id"=226;
INSERT INTO mame_field_assertions VALUES ('required:device:type',(changes()=0));
UPDATE OR IGNORE mame_device_instances SET "name"=NULL WHERE "source_element_id"=227;
INSERT INTO mame_field_assertions VALUES ('required:instance:name',(changes()=0));
UPDATE OR IGNORE mame_device_instances SET "briefname"=NULL WHERE "source_element_id"=227;
INSERT INTO mame_field_assertions VALUES ('required:instance:briefname',(changes()=0));
UPDATE OR IGNORE mame_device_extensions SET "name"=NULL WHERE "source_element_id"=228;
INSERT INTO mame_field_assertions VALUES ('required:extension:name',(changes()=0));
UPDATE OR IGNORE mame_slots SET "name"=NULL WHERE "source_element_id"=229;
INSERT INTO mame_field_assertions VALUES ('required:slot:name',(changes()=0));
UPDATE OR IGNORE mame_slot_options SET "name"=NULL WHERE "source_element_id"=230;
INSERT INTO mame_field_assertions VALUES ('required:slotoption:name',(changes()=0));
UPDATE OR IGNORE mame_slot_options SET "devname"=NULL WHERE "source_element_id"=230;
INSERT INTO mame_field_assertions VALUES ('required:slotoption:devname',(changes()=0));
UPDATE OR IGNORE mame_softwarelist_references SET "tag"=NULL WHERE "source_element_id"=231;
INSERT INTO mame_field_assertions VALUES ('required:softwarelist:tag',(changes()=0));
UPDATE OR IGNORE mame_softwarelist_references SET "name"=NULL WHERE "source_element_id"=231;
INSERT INTO mame_field_assertions VALUES ('required:softwarelist:name',(changes()=0));
UPDATE OR IGNORE mame_softwarelist_references SET "status"=NULL WHERE "source_element_id"=231;
INSERT INTO mame_field_assertions VALUES ('required:softwarelist:status',(changes()=0));
UPDATE OR IGNORE mame_softwarelist_references SET "status"='' WHERE "source_element_id"=231;
INSERT INTO mame_field_assertions VALUES ('enum:softwarelist:status',(changes()=0));
UPDATE OR IGNORE mame_ram_options SET "name"=NULL WHERE "source_element_id"=232;
INSERT INTO mame_field_assertions VALUES ('required:ramoption:name',(changes()=0));

-- Shared table spellings do not substitute for independent wire contexts.
-- Bind each constructed source owner to its actual typed parent, separately
-- from the field assertions above and the independent DTD inventory checker.
INSERT INTO mame_field_assertions VALUES ('context:dipswitch',(SELECT kind='dipswitch' FROM mame_switches WHERE source_element_id=210));
INSERT INTO mame_field_assertions VALUES ('context:configuration',(SELECT kind='configuration' FROM mame_switches WHERE source_element_id=213));
INSERT INTO mame_field_assertions VALUES ('context:diplocation',(SELECT parent.kind='dipswitch' FROM mame_switch_locations AS child JOIN mame_switches AS parent ON parent.source_element_id=child.switch_id WHERE child.source_element_id=211));
INSERT INTO mame_field_assertions VALUES ('context:conflocation',(SELECT parent.kind='configuration' FROM mame_switch_locations AS child JOIN mame_switches AS parent ON parent.source_element_id=child.switch_id WHERE child.source_element_id=214));
INSERT INTO mame_field_assertions VALUES ('context:dipvalue',(SELECT parent.kind='dipswitch' FROM mame_switch_values AS child JOIN mame_switches AS parent ON parent.source_element_id=child.switch_id WHERE child.source_element_id=212));
INSERT INTO mame_field_assertions VALUES ('context:confsetting',(SELECT parent.kind='configuration' FROM mame_switch_values AS child JOIN mame_switches AS parent ON parent.source_element_id=child.switch_id WHERE child.source_element_id=215));
INSERT INTO mame_field_assertions VALUES ('context:dipswitch.condition',(SELECT switch_id=210 FROM mame_switch_conditions WHERE source_element_id=216));
INSERT INTO mame_field_assertions VALUES ('context:configuration.condition',(SELECT switch_id=213 FROM mame_switch_conditions WHERE source_element_id=217));
INSERT INTO mame_field_assertions VALUES ('context:dipvalue.condition',(SELECT value_id=212 FROM mame_switch_value_conditions WHERE source_element_id=218));
INSERT INTO mame_field_assertions VALUES ('context:confsetting.condition',(SELECT value_id=215 FROM mame_switch_value_conditions WHERE source_element_id=219));
INSERT INTO mame_field_assertions VALUES ('context:adjuster.condition',(SELECT adjuster_id=222 FROM mame_adjuster_conditions WHERE source_element_id=223));
SAVEPOINT spelling_independence;
UPDATE mame_disk_compatibility SET writeable=1 WHERE media_entry_id=202;
INSERT INTO mame_field_assertions VALUES ('spelling:writable-writeable-independent',(SELECT native.writable=0 AND compatibility.writeable=1 AND native.writable_specified=1 FROM mame_disks AS native JOIN mame_disk_compatibility AS compatibility USING(media_entry_id) WHERE media_entry_id=202));
INSERT INTO mame_field_assertions VALUES ('spelling:writable-writeable-positions',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
ROLLBACK TO spelling_independence;
RELEASE spelling_independence;

-- Twenty-two underlying default-presence fields, tested independently in
-- all 24 wire contexts (DIP/configuration location/value defaults included).
SAVEPOINT default_state;
DELETE FROM mame_document_facts_attribute_positions WHERE "edition_id"=1 AND field_kind='debug' AND field_occurrence=0;
UPDATE mame_documents SET "debug_specified"=0 WHERE "edition_id"=1;
INSERT INTO mame_field_assertions VALUES ('default-absent:mame:debug',(EXISTS(SELECT 1 FROM mame_documents WHERE "edition_id"=1 AND "debug" IS 0 AND "debug_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_document_facts_attribute_positions WHERE "edition_id"=1 AND field_kind='debug' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:mame:debug',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_documents SET "debug"=1 WHERE "edition_id"=1;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:mame:debug',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_machines_attribute_positions WHERE "set_id"=100 AND field_kind='isbios' AND field_occurrence=0;
UPDATE mame_machines SET "isbios_specified"=0 WHERE "set_id"=100;
INSERT INTO mame_field_assertions VALUES ('default-absent:machine:isbios',(EXISTS(SELECT 1 FROM mame_machines WHERE "set_id"=100 AND "isbios" IS 0 AND "isbios_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_machines_attribute_positions WHERE "set_id"=100 AND field_kind='isbios' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:machine:isbios',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_machines SET "isbios"=1 WHERE "set_id"=100;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:machine:isbios',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_machines_attribute_positions WHERE "set_id"=100 AND field_kind='isdevice' AND field_occurrence=0;
UPDATE mame_machines SET "isdevice_specified"=0 WHERE "set_id"=100;
INSERT INTO mame_field_assertions VALUES ('default-absent:machine:isdevice',(EXISTS(SELECT 1 FROM mame_machines WHERE "set_id"=100 AND "isdevice" IS 0 AND "isdevice_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_machines_attribute_positions WHERE "set_id"=100 AND field_kind='isdevice' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:machine:isdevice',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_machines SET "isdevice"=1 WHERE "set_id"=100;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:machine:isdevice',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_machines_attribute_positions WHERE "set_id"=100 AND field_kind='ismechanical' AND field_occurrence=0;
UPDATE mame_machines SET "ismechanical_specified"=0 WHERE "set_id"=100;
INSERT INTO mame_field_assertions VALUES ('default-absent:machine:ismechanical',(EXISTS(SELECT 1 FROM mame_machines WHERE "set_id"=100 AND "ismechanical" IS 0 AND "ismechanical_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_machines_attribute_positions WHERE "set_id"=100 AND field_kind='ismechanical' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:machine:ismechanical',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_machines SET "ismechanical"=1 WHERE "set_id"=100;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:machine:ismechanical',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_machines_attribute_positions WHERE "set_id"=100 AND field_kind='runnable' AND field_occurrence=0;
UPDATE mame_machines SET "runnable_specified"=0 WHERE "set_id"=100;
INSERT INTO mame_field_assertions VALUES ('default-absent:machine:runnable',(EXISTS(SELECT 1 FROM mame_machines WHERE "set_id"=100 AND "runnable" IS 1 AND "runnable_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_machines_attribute_positions WHERE "set_id"=100 AND field_kind='runnable' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:machine:runnable',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_machines SET "runnable"=0 WHERE "set_id"=100;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:machine:runnable',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_bios_sets_attribute_positions WHERE "source_element_id"=200 AND field_kind='default' AND field_occurrence=0;
UPDATE mame_bios_sets SET "default_specified"=0 WHERE "source_element_id"=200;
INSERT INTO mame_field_assertions VALUES ('default-absent:biosset:default',(EXISTS(SELECT 1 FROM mame_bios_sets WHERE "source_element_id"=200 AND "default" IS 0 AND "default_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_bios_sets_attribute_positions WHERE "source_element_id"=200 AND field_kind='default' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:biosset:default',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_bios_sets SET "default"=1 WHERE "source_element_id"=200;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:biosset:default',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='status' AND field_occurrence=0;
UPDATE mame_roms SET "status_specified"=0 WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('default-absent:rom:status',(EXISTS(SELECT 1 FROM mame_roms WHERE "media_entry_id"=201 AND "status" IS 'good' AND "status_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='status' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:rom:status',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_roms SET "status"='baddump' WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:rom:status',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='optional' AND field_occurrence=0;
UPDATE mame_roms SET "optional_specified"=0 WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('default-absent:rom:optional',(EXISTS(SELECT 1 FROM mame_roms WHERE "media_entry_id"=201 AND "optional" IS 0 AND "optional_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='optional' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:rom:optional',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_roms SET "optional"=1 WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:rom:optional',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_disk_claims_attribute_positions WHERE "media_entry_id"=202 AND field_kind='writable' AND field_occurrence=0;
UPDATE mame_disks SET "writable_specified"=0 WHERE "media_entry_id"=202;
INSERT INTO mame_field_assertions VALUES ('default-absent:disk:writable',(EXISTS(SELECT 1 FROM mame_disks WHERE "media_entry_id"=202 AND "writable" IS 0 AND "writable_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions WHERE "media_entry_id"=202 AND field_kind='writable' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:disk:writable',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_disks SET "writable"=1 WHERE "media_entry_id"=202;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:disk:writable',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_disk_claims_attribute_positions WHERE "media_entry_id"=202 AND field_kind='status' AND field_occurrence=0;
UPDATE mame_disks SET "status_specified"=0 WHERE "media_entry_id"=202;
INSERT INTO mame_field_assertions VALUES ('default-absent:disk:status',(EXISTS(SELECT 1 FROM mame_disks WHERE "media_entry_id"=202 AND "status" IS 'good' AND "status_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions WHERE "media_entry_id"=202 AND field_kind='status' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:disk:status',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_disks SET "status"='baddump' WHERE "media_entry_id"=202;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:disk:status',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_disk_claims_attribute_positions WHERE "media_entry_id"=202 AND field_kind='optional' AND field_occurrence=0;
UPDATE mame_disks SET "optional_specified"=0 WHERE "media_entry_id"=202;
INSERT INTO mame_field_assertions VALUES ('default-absent:disk:optional',(EXISTS(SELECT 1 FROM mame_disks WHERE "media_entry_id"=202 AND "optional" IS 0 AND "optional_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions WHERE "media_entry_id"=202 AND field_kind='optional' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:disk:optional',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_disks SET "optional"=1 WHERE "media_entry_id"=202;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:disk:optional',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='flipx' AND field_occurrence=0;
UPDATE mame_displays SET "flipx_specified"=0 WHERE "source_element_id"=206;
INSERT INTO mame_field_assertions VALUES ('default-absent:display:flipx',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "flipx" IS 0 AND "flipx_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='flipx' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:display:flipx',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_displays SET "flipx"=1 WHERE "source_element_id"=206;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:display:flipx',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_machine_inputs_attribute_positions WHERE "source_element_id"=208 AND field_kind='service' AND field_occurrence=0;
UPDATE mame_inputs SET "service_specified"=0 WHERE "source_element_id"=208;
INSERT INTO mame_field_assertions VALUES ('default-absent:input:service',(EXISTS(SELECT 1 FROM mame_inputs WHERE "source_element_id"=208 AND "service" IS 0 AND "service_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_machine_inputs_attribute_positions WHERE "source_element_id"=208 AND field_kind='service' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:input:service',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_inputs SET "service"=1 WHERE "source_element_id"=208;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:input:service',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_machine_inputs_attribute_positions WHERE "source_element_id"=208 AND field_kind='tilt' AND field_occurrence=0;
UPDATE mame_inputs SET "tilt_specified"=0 WHERE "source_element_id"=208;
INSERT INTO mame_field_assertions VALUES ('default-absent:input:tilt',(EXISTS(SELECT 1 FROM mame_inputs WHERE "source_element_id"=208 AND "tilt" IS 0 AND "tilt_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_machine_inputs_attribute_positions WHERE "source_element_id"=208 AND field_kind='tilt' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:input:tilt',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_inputs SET "tilt"=1 WHERE "source_element_id"=208;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:input:tilt',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='reverse' AND field_occurrence=0;
UPDATE mame_input_controls SET "reverse_specified"=0 WHERE "source_element_id"=209;
INSERT INTO mame_field_assertions VALUES ('default-absent:control:reverse',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "reverse" IS 0 AND "reverse_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='reverse' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:control:reverse',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_input_controls SET "reverse"=1 WHERE "source_element_id"=209;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:control:reverse',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM machine_switch_locations_attribute_positions WHERE "source_element_id"=211 AND field_kind='inverted' AND field_occurrence=0;
UPDATE mame_switch_locations SET "inverted_specified"=0 WHERE "source_element_id"=211;
INSERT INTO mame_field_assertions VALUES ('default-absent:diplocation:inverted',(EXISTS(SELECT 1 FROM mame_switch_locations WHERE "source_element_id"=211 AND "inverted" IS 0 AND "inverted_specified"=0) AND NOT EXISTS(SELECT 1 FROM machine_switch_locations_attribute_positions WHERE "source_element_id"=211 AND field_kind='inverted' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:diplocation:inverted',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_switch_locations SET "inverted"=1 WHERE "source_element_id"=211;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:diplocation:inverted',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM machine_switch_values_attribute_positions WHERE "source_element_id"=212 AND field_kind='default' AND field_occurrence=0;
UPDATE mame_switch_values SET "default_specified"=0 WHERE "source_element_id"=212;
INSERT INTO mame_field_assertions VALUES ('default-absent:dipvalue:default',(EXISTS(SELECT 1 FROM mame_switch_values WHERE "source_element_id"=212 AND "default" IS 0 AND "default_specified"=0) AND NOT EXISTS(SELECT 1 FROM machine_switch_values_attribute_positions WHERE "source_element_id"=212 AND field_kind='default' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:dipvalue:default',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_switch_values SET "default"=1 WHERE "source_element_id"=212;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:dipvalue:default',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM machine_switch_locations_attribute_positions WHERE "source_element_id"=214 AND field_kind='inverted' AND field_occurrence=0;
UPDATE mame_switch_locations SET "inverted_specified"=0 WHERE "source_element_id"=214;
INSERT INTO mame_field_assertions VALUES ('default-absent:conflocation:inverted',(EXISTS(SELECT 1 FROM mame_switch_locations WHERE "source_element_id"=214 AND "inverted" IS 0 AND "inverted_specified"=0) AND NOT EXISTS(SELECT 1 FROM machine_switch_locations_attribute_positions WHERE "source_element_id"=214 AND field_kind='inverted' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:conflocation:inverted',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_switch_locations SET "inverted"=1 WHERE "source_element_id"=214;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:conflocation:inverted',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM machine_switch_values_attribute_positions WHERE "source_element_id"=215 AND field_kind='default' AND field_occurrence=0;
UPDATE mame_switch_values SET "default_specified"=0 WHERE "source_element_id"=215;
INSERT INTO mame_field_assertions VALUES ('default-absent:confsetting:default',(EXISTS(SELECT 1 FROM mame_switch_values WHERE "source_element_id"=215 AND "default" IS 0 AND "default_specified"=0) AND NOT EXISTS(SELECT 1 FROM machine_switch_values_attribute_positions WHERE "source_element_id"=215 AND field_kind='default' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:confsetting:default',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_switch_values SET "default"=1 WHERE "source_element_id"=215;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:confsetting:default',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_machine_drivers_attribute_positions WHERE "source_element_id"=224 AND field_kind='requiresartwork' AND field_occurrence=0;
UPDATE mame_drivers SET "requiresartwork_specified"=0 WHERE "source_element_id"=224;
INSERT INTO mame_field_assertions VALUES ('default-absent:driver:requiresartwork',(EXISTS(SELECT 1 FROM mame_drivers WHERE "source_element_id"=224 AND "requiresartwork" IS 0 AND "requiresartwork_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions WHERE "source_element_id"=224 AND field_kind='requiresartwork' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:driver:requiresartwork',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_drivers SET "requiresartwork"=1 WHERE "source_element_id"=224;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:driver:requiresartwork',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_machine_drivers_attribute_positions WHERE "source_element_id"=224 AND field_kind='unofficial' AND field_occurrence=0;
UPDATE mame_drivers SET "unofficial_specified"=0 WHERE "source_element_id"=224;
INSERT INTO mame_field_assertions VALUES ('default-absent:driver:unofficial',(EXISTS(SELECT 1 FROM mame_drivers WHERE "source_element_id"=224 AND "unofficial" IS 0 AND "unofficial_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions WHERE "source_element_id"=224 AND field_kind='unofficial' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:driver:unofficial',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_drivers SET "unofficial"=1 WHERE "source_element_id"=224;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:driver:unofficial',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_machine_drivers_attribute_positions WHERE "source_element_id"=224 AND field_kind='nosoundhardware' AND field_occurrence=0;
UPDATE mame_drivers SET "nosoundhardware_specified"=0 WHERE "source_element_id"=224;
INSERT INTO mame_field_assertions VALUES ('default-absent:driver:nosoundhardware',(EXISTS(SELECT 1 FROM mame_drivers WHERE "source_element_id"=224 AND "nosoundhardware" IS 0 AND "nosoundhardware_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions WHERE "source_element_id"=224 AND field_kind='nosoundhardware' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:driver:nosoundhardware',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_drivers SET "nosoundhardware"=1 WHERE "source_element_id"=224;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:driver:nosoundhardware',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_machine_drivers_attribute_positions WHERE "source_element_id"=224 AND field_kind='incomplete' AND field_occurrence=0;
UPDATE mame_drivers SET "incomplete_specified"=0 WHERE "source_element_id"=224;
INSERT INTO mame_field_assertions VALUES ('default-absent:driver:incomplete',(EXISTS(SELECT 1 FROM mame_drivers WHERE "source_element_id"=224 AND "incomplete" IS 0 AND "incomplete_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions WHERE "source_element_id"=224 AND field_kind='incomplete' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:driver:incomplete',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_drivers SET "incomplete"=1 WHERE "source_element_id"=224;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:driver:incomplete',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;
SAVEPOINT default_state;
DELETE FROM mame_machine_slot_options_attribute_positions WHERE "source_element_id"=230 AND field_kind='default' AND field_occurrence=0;
UPDATE mame_slot_options SET "default_specified"=0 WHERE "source_element_id"=230;
INSERT INTO mame_field_assertions VALUES ('default-absent:slotoption:default',(EXISTS(SELECT 1 FROM mame_slot_options WHERE "source_element_id"=230 AND "default" IS 0 AND "default_specified"=0) AND NOT EXISTS(SELECT 1 FROM mame_machine_slot_options_attribute_positions WHERE "source_element_id"=230 AND field_kind='default' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('default-absence-audit:slotoption:default',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_slot_options SET "default"=1 WHERE "source_element_id"=230;
INSERT INTO mame_field_assertions VALUES ('unspecified-nondefault:slotoption:default',(changes()=0));
ROLLBACK TO default_state;
RELEASE default_state;

-- Nullable source fields distinguish omission from supplied empty text;
-- enum/boolean omissions are NULL and have no invented default/position.
SAVEPOINT optional_state;
DELETE FROM mame_document_facts_attribute_positions WHERE "edition_id"=1 AND field_kind='build' AND field_occurrence=0;
UPDATE mame_documents SET "build"=NULL WHERE "edition_id"=1;
INSERT INTO mame_field_assertions VALUES ('optional-absent:mame:build',(EXISTS(SELECT 1 FROM mame_documents WHERE "edition_id"=1 AND "build" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_document_facts_attribute_positions WHERE "edition_id"=1 AND field_kind='build' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:mame:build',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_documents SET "build"='' WHERE "edition_id"=1;
INSERT INTO mame_document_facts_attribute_positions ("edition_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (1,'build',0,0,1,10);
INSERT INTO mame_field_assertions VALUES ('optional-empty:mame:build',(EXISTS(SELECT 1 FROM mame_documents WHERE "edition_id"=1 AND "build"='') AND EXISTS(SELECT 1 FROM mame_document_facts_attribute_positions WHERE "edition_id"=1 AND field_kind='build' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machines_attribute_positions WHERE "set_id"=100 AND field_kind='sourcefile' AND field_occurrence=0;
UPDATE mame_machines SET "sourcefile"=NULL WHERE "set_id"=100;
INSERT INTO mame_field_assertions VALUES ('optional-absent:machine:sourcefile',(EXISTS(SELECT 1 FROM mame_machines WHERE "set_id"=100 AND "sourcefile" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machines_attribute_positions WHERE "set_id"=100 AND field_kind='sourcefile' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:machine:sourcefile',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_machines SET "sourcefile"='' WHERE "set_id"=100;
INSERT INTO mame_machines_attribute_positions ("set_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (100,'sourcefile',0,1,2,22);
INSERT INTO mame_field_assertions VALUES ('optional-empty:machine:sourcefile',(EXISTS(SELECT 1 FROM mame_machines WHERE "set_id"=100 AND "sourcefile"='') AND EXISTS(SELECT 1 FROM mame_machines_attribute_positions WHERE "set_id"=100 AND field_kind='sourcefile' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='bios' AND field_occurrence=0;
UPDATE mame_roms SET "bios"=NULL WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('optional-absent:rom:bios',(EXISTS(SELECT 1 FROM mame_roms WHERE "media_entry_id"=201 AND "bios" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='bios' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:rom:bios',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_roms SET "bios"='' WHERE "media_entry_id"=201;
INSERT INTO mame_rom_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'bios',0,1,11,22);
INSERT INTO mame_field_assertions VALUES ('optional-empty:rom:bios',(EXISTS(SELECT 1 FROM mame_roms WHERE "media_entry_id"=201 AND "bios"='') AND EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='bios' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='size' AND field_occurrence=0;
UPDATE mame_roms SET "size_text"=NULL WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('optional-absent:rom:size',(EXISTS(SELECT 1 FROM mame_roms WHERE "media_entry_id"=201 AND "size_text" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='size' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:rom:size',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_roms SET "size_text"='' WHERE "media_entry_id"=201;
INSERT INTO mame_rom_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'size',0,2,11,34);
INSERT INTO mame_field_assertions VALUES ('optional-empty:rom:size',(EXISTS(SELECT 1 FROM mame_roms WHERE "media_entry_id"=201 AND "size_text"='') AND EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='size' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='region' AND field_occurrence=0;
UPDATE mame_roms SET "region"=NULL WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('optional-absent:rom:region',(EXISTS(SELECT 1 FROM mame_roms WHERE "media_entry_id"=201 AND "region" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='region' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:rom:region',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_roms SET "region"='' WHERE "media_entry_id"=201;
INSERT INTO mame_rom_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'region',0,6,11,82);
INSERT INTO mame_field_assertions VALUES ('optional-empty:rom:region',(EXISTS(SELECT 1 FROM mame_roms WHERE "media_entry_id"=201 AND "region"='') AND EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='region' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='offset' AND field_occurrence=0;
UPDATE mame_roms SET "offset"=NULL WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('optional-absent:rom:offset',(EXISTS(SELECT 1 FROM mame_roms WHERE "media_entry_id"=201 AND "offset" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='offset' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:rom:offset',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_roms SET "offset"='' WHERE "media_entry_id"=201;
INSERT INTO mame_rom_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'offset',0,7,11,94);
INSERT INTO mame_field_assertions VALUES ('optional-empty:rom:offset',(EXISTS(SELECT 1 FROM mame_roms WHERE "media_entry_id"=201 AND "offset"='') AND EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='offset' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_disk_claims_attribute_positions WHERE "media_entry_id"=202 AND field_kind='region' AND field_occurrence=0;
UPDATE mame_disks SET "region"=NULL WHERE "media_entry_id"=202;
INSERT INTO mame_field_assertions VALUES ('optional-absent:disk:region',(EXISTS(SELECT 1 FROM mame_disks WHERE "media_entry_id"=202 AND "region" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions WHERE "media_entry_id"=202 AND field_kind='region' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:disk:region',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_disks SET "region"='' WHERE "media_entry_id"=202;
INSERT INTO mame_disk_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (202,'region',0,3,12,46);
INSERT INTO mame_field_assertions VALUES ('optional-empty:disk:region',(EXISTS(SELECT 1 FROM mame_disks WHERE "media_entry_id"=202 AND "region"='') AND EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions WHERE "media_entry_id"=202 AND field_kind='region' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_disk_claims_attribute_positions WHERE "media_entry_id"=202 AND field_kind='index' AND field_occurrence=0;
UPDATE mame_disks SET "index"=NULL WHERE "media_entry_id"=202;
INSERT INTO mame_field_assertions VALUES ('optional-absent:disk:index',(EXISTS(SELECT 1 FROM mame_disks WHERE "media_entry_id"=202 AND "index" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions WHERE "media_entry_id"=202 AND field_kind='index' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:disk:index',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_disks SET "index"='' WHERE "media_entry_id"=202;
INSERT INTO mame_disk_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (202,'index',0,4,12,58);
INSERT INTO mame_field_assertions VALUES ('optional-empty:disk:index',(EXISTS(SELECT 1 FROM mame_disks WHERE "media_entry_id"=202 AND "index"='') AND EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions WHERE "media_entry_id"=202 AND field_kind='index' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_chips_attribute_positions WHERE "source_element_id"=205 AND field_kind='tag' AND field_occurrence=0;
UPDATE mame_chips SET "tag"=NULL WHERE "source_element_id"=205;
INSERT INTO mame_field_assertions VALUES ('optional-absent:chip:tag',(EXISTS(SELECT 1 FROM mame_chips WHERE "source_element_id"=205 AND "tag" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_chips_attribute_positions WHERE "source_element_id"=205 AND field_kind='tag' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:chip:tag',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_chips SET "tag"='' WHERE "source_element_id"=205;
INSERT INTO mame_machine_chips_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (205,'tag',0,1,15,22);
INSERT INTO mame_field_assertions VALUES ('optional-empty:chip:tag',(EXISTS(SELECT 1 FROM mame_chips WHERE "source_element_id"=205 AND "tag"='') AND EXISTS(SELECT 1 FROM mame_machine_chips_attribute_positions WHERE "source_element_id"=205 AND field_kind='tag' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_chips_attribute_positions WHERE "source_element_id"=205 AND field_kind='clock' AND field_occurrence=0;
UPDATE mame_chips SET "clock"=NULL WHERE "source_element_id"=205;
INSERT INTO mame_field_assertions VALUES ('optional-absent:chip:clock',(EXISTS(SELECT 1 FROM mame_chips WHERE "source_element_id"=205 AND "clock" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_chips_attribute_positions WHERE "source_element_id"=205 AND field_kind='clock' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:chip:clock',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_chips SET "clock"='' WHERE "source_element_id"=205;
INSERT INTO mame_machine_chips_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (205,'clock',0,3,15,46);
INSERT INTO mame_field_assertions VALUES ('optional-empty:chip:clock',(EXISTS(SELECT 1 FROM mame_chips WHERE "source_element_id"=205 AND "clock"='') AND EXISTS(SELECT 1 FROM mame_machine_chips_attribute_positions WHERE "source_element_id"=205 AND field_kind='clock' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='tag' AND field_occurrence=0;
UPDATE mame_displays SET "tag"=NULL WHERE "source_element_id"=206;
INSERT INTO mame_field_assertions VALUES ('optional-absent:display:tag',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "tag" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='tag' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:display:tag',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_displays SET "tag"='' WHERE "source_element_id"=206;
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'tag',0,0,16,10);
INSERT INTO mame_field_assertions VALUES ('optional-empty:display:tag',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "tag"='') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='tag' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='rotate' AND field_occurrence=0;
UPDATE mame_displays SET "rotate"=NULL WHERE "source_element_id"=206;
INSERT INTO mame_field_assertions VALUES ('optional-absent:display:rotate',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "rotate" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='rotate' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:display:rotate',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_displays SET "rotate"='' WHERE "source_element_id"=206;
INSERT INTO mame_field_assertions VALUES ('optional-invalid:display:rotate',(changes()=0));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='width' AND field_occurrence=0;
UPDATE mame_displays SET "width"=NULL WHERE "source_element_id"=206;
INSERT INTO mame_field_assertions VALUES ('optional-absent:display:width',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "width" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='width' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:display:width',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_displays SET "width"='' WHERE "source_element_id"=206;
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'width',0,4,16,58);
INSERT INTO mame_field_assertions VALUES ('optional-empty:display:width',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "width"='') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='width' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='height' AND field_occurrence=0;
UPDATE mame_displays SET "height"=NULL WHERE "source_element_id"=206;
INSERT INTO mame_field_assertions VALUES ('optional-absent:display:height',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "height" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='height' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:display:height',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_displays SET "height"='' WHERE "source_element_id"=206;
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'height',0,5,16,70);
INSERT INTO mame_field_assertions VALUES ('optional-empty:display:height',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "height"='') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='height' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='pixclock' AND field_occurrence=0;
UPDATE mame_displays SET "pixclock"=NULL WHERE "source_element_id"=206;
INSERT INTO mame_field_assertions VALUES ('optional-absent:display:pixclock',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "pixclock" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='pixclock' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:display:pixclock',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_displays SET "pixclock"='' WHERE "source_element_id"=206;
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'pixclock',0,7,16,94);
INSERT INTO mame_field_assertions VALUES ('optional-empty:display:pixclock',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "pixclock"='') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='pixclock' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='htotal' AND field_occurrence=0;
UPDATE mame_displays SET "htotal"=NULL WHERE "source_element_id"=206;
INSERT INTO mame_field_assertions VALUES ('optional-absent:display:htotal',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "htotal" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='htotal' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:display:htotal',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_displays SET "htotal"='' WHERE "source_element_id"=206;
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'htotal',0,8,16,106);
INSERT INTO mame_field_assertions VALUES ('optional-empty:display:htotal',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "htotal"='') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='htotal' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='hbend' AND field_occurrence=0;
UPDATE mame_displays SET "hbend"=NULL WHERE "source_element_id"=206;
INSERT INTO mame_field_assertions VALUES ('optional-absent:display:hbend',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "hbend" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='hbend' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:display:hbend',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_displays SET "hbend"='' WHERE "source_element_id"=206;
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'hbend',0,9,16,118);
INSERT INTO mame_field_assertions VALUES ('optional-empty:display:hbend',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "hbend"='') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='hbend' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='hbstart' AND field_occurrence=0;
UPDATE mame_displays SET "hbstart"=NULL WHERE "source_element_id"=206;
INSERT INTO mame_field_assertions VALUES ('optional-absent:display:hbstart',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "hbstart" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='hbstart' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:display:hbstart',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_displays SET "hbstart"='' WHERE "source_element_id"=206;
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'hbstart',0,10,16,130);
INSERT INTO mame_field_assertions VALUES ('optional-empty:display:hbstart',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "hbstart"='') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='hbstart' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='vtotal' AND field_occurrence=0;
UPDATE mame_displays SET "vtotal"=NULL WHERE "source_element_id"=206;
INSERT INTO mame_field_assertions VALUES ('optional-absent:display:vtotal',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "vtotal" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='vtotal' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:display:vtotal',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_displays SET "vtotal"='' WHERE "source_element_id"=206;
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'vtotal',0,11,16,142);
INSERT INTO mame_field_assertions VALUES ('optional-empty:display:vtotal',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "vtotal"='') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='vtotal' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='vbend' AND field_occurrence=0;
UPDATE mame_displays SET "vbend"=NULL WHERE "source_element_id"=206;
INSERT INTO mame_field_assertions VALUES ('optional-absent:display:vbend',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "vbend" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='vbend' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:display:vbend',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_displays SET "vbend"='' WHERE "source_element_id"=206;
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'vbend',0,12,16,154);
INSERT INTO mame_field_assertions VALUES ('optional-empty:display:vbend',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "vbend"='') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='vbend' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='vbstart' AND field_occurrence=0;
UPDATE mame_displays SET "vbstart"=NULL WHERE "source_element_id"=206;
INSERT INTO mame_field_assertions VALUES ('optional-absent:display:vbstart',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "vbstart" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='vbstart' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:display:vbstart',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_displays SET "vbstart"='' WHERE "source_element_id"=206;
INSERT INTO mame_machine_displays_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (206,'vbstart',0,13,16,166);
INSERT INTO mame_field_assertions VALUES ('optional-empty:display:vbstart',(EXISTS(SELECT 1 FROM mame_displays WHERE "source_element_id"=206 AND "vbstart"='') AND EXISTS(SELECT 1 FROM mame_machine_displays_attribute_positions WHERE "source_element_id"=206 AND field_kind='vbstart' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_inputs_attribute_positions WHERE "source_element_id"=208 AND field_kind='coins' AND field_occurrence=0;
UPDATE mame_inputs SET "coins"=NULL WHERE "source_element_id"=208;
INSERT INTO mame_field_assertions VALUES ('optional-absent:input:coins',(EXISTS(SELECT 1 FROM mame_inputs WHERE "source_element_id"=208 AND "coins" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_inputs_attribute_positions WHERE "source_element_id"=208 AND field_kind='coins' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:input:coins',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_inputs SET "coins"='' WHERE "source_element_id"=208;
INSERT INTO mame_machine_inputs_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (208,'coins',0,3,18,46);
INSERT INTO mame_field_assertions VALUES ('optional-empty:input:coins',(EXISTS(SELECT 1 FROM mame_inputs WHERE "source_element_id"=208 AND "coins"='') AND EXISTS(SELECT 1 FROM mame_machine_inputs_attribute_positions WHERE "source_element_id"=208 AND field_kind='coins' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='player' AND field_occurrence=0;
UPDATE mame_input_controls SET "player"=NULL WHERE "source_element_id"=209;
INSERT INTO mame_field_assertions VALUES ('optional-absent:control:player',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "player" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='player' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:control:player',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_input_controls SET "player"='' WHERE "source_element_id"=209;
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'player',0,1,19,22);
INSERT INTO mame_field_assertions VALUES ('optional-empty:control:player',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "player"='') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='player' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='buttons' AND field_occurrence=0;
UPDATE mame_input_controls SET "buttons"=NULL WHERE "source_element_id"=209;
INSERT INTO mame_field_assertions VALUES ('optional-absent:control:buttons',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "buttons" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='buttons' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:control:buttons',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_input_controls SET "buttons"='' WHERE "source_element_id"=209;
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'buttons',0,2,19,34);
INSERT INTO mame_field_assertions VALUES ('optional-empty:control:buttons',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "buttons"='') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='buttons' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='minimum' AND field_occurrence=0;
UPDATE mame_input_controls SET "minimum"=NULL WHERE "source_element_id"=209;
INSERT INTO mame_field_assertions VALUES ('optional-absent:control:minimum',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "minimum" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='minimum' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:control:minimum',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_input_controls SET "minimum"='' WHERE "source_element_id"=209;
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'minimum',0,3,19,46);
INSERT INTO mame_field_assertions VALUES ('optional-empty:control:minimum',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "minimum"='') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='minimum' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='maximum' AND field_occurrence=0;
UPDATE mame_input_controls SET "maximum"=NULL WHERE "source_element_id"=209;
INSERT INTO mame_field_assertions VALUES ('optional-absent:control:maximum',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "maximum" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='maximum' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:control:maximum',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_input_controls SET "maximum"='' WHERE "source_element_id"=209;
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'maximum',0,4,19,58);
INSERT INTO mame_field_assertions VALUES ('optional-empty:control:maximum',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "maximum"='') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='maximum' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='sensitivity' AND field_occurrence=0;
UPDATE mame_input_controls SET "sensitivity"=NULL WHERE "source_element_id"=209;
INSERT INTO mame_field_assertions VALUES ('optional-absent:control:sensitivity',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "sensitivity" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='sensitivity' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:control:sensitivity',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_input_controls SET "sensitivity"='' WHERE "source_element_id"=209;
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'sensitivity',0,5,19,70);
INSERT INTO mame_field_assertions VALUES ('optional-empty:control:sensitivity',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "sensitivity"='') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='sensitivity' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='keydelta' AND field_occurrence=0;
UPDATE mame_input_controls SET "keydelta"=NULL WHERE "source_element_id"=209;
INSERT INTO mame_field_assertions VALUES ('optional-absent:control:keydelta',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "keydelta" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='keydelta' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:control:keydelta',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_input_controls SET "keydelta"='' WHERE "source_element_id"=209;
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'keydelta',0,6,19,82);
INSERT INTO mame_field_assertions VALUES ('optional-empty:control:keydelta',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "keydelta"='') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='keydelta' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='ways' AND field_occurrence=0;
UPDATE mame_input_controls SET "ways"=NULL WHERE "source_element_id"=209;
INSERT INTO mame_field_assertions VALUES ('optional-absent:control:ways',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "ways" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='ways' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:control:ways',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_input_controls SET "ways"='' WHERE "source_element_id"=209;
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'ways',0,8,19,106);
INSERT INTO mame_field_assertions VALUES ('optional-empty:control:ways',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "ways"='') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='ways' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='ways2' AND field_occurrence=0;
UPDATE mame_input_controls SET "ways2"=NULL WHERE "source_element_id"=209;
INSERT INTO mame_field_assertions VALUES ('optional-absent:control:ways2',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "ways2" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='ways2' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:control:ways2',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_input_controls SET "ways2"='' WHERE "source_element_id"=209;
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'ways2',0,9,19,118);
INSERT INTO mame_field_assertions VALUES ('optional-empty:control:ways2',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "ways2"='') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='ways2' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='ways3' AND field_occurrence=0;
UPDATE mame_input_controls SET "ways3"=NULL WHERE "source_element_id"=209;
INSERT INTO mame_field_assertions VALUES ('optional-absent:control:ways3',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "ways3" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='ways3' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:control:ways3',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_input_controls SET "ways3"='' WHERE "source_element_id"=209;
INSERT INTO mame_machine_input_controls_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (209,'ways3',0,10,19,130);
INSERT INTO mame_field_assertions VALUES ('optional-empty:control:ways3',(EXISTS(SELECT 1 FROM mame_input_controls WHERE "source_element_id"=209 AND "ways3"='') AND EXISTS(SELECT 1 FROM mame_machine_input_controls_attribute_positions WHERE "source_element_id"=209 AND field_kind='ways3' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_drivers_attribute_positions WHERE "source_element_id"=224 AND field_kind='cocktail' AND field_occurrence=0;
UPDATE mame_drivers SET "cocktail"=NULL WHERE "source_element_id"=224;
INSERT INTO mame_field_assertions VALUES ('optional-absent:driver:cocktail',(EXISTS(SELECT 1 FROM mame_drivers WHERE "source_element_id"=224 AND "cocktail" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_drivers_attribute_positions WHERE "source_element_id"=224 AND field_kind='cocktail' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:driver:cocktail',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_drivers SET "cocktail"='' WHERE "source_element_id"=224;
INSERT INTO mame_field_assertions VALUES ('optional-invalid:driver:cocktail',(changes()=0));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_features_attribute_positions WHERE "source_element_id"=225 AND field_kind='status' AND field_occurrence=0;
UPDATE mame_features SET "status"=NULL WHERE "source_element_id"=225;
INSERT INTO mame_field_assertions VALUES ('optional-absent:feature:status',(EXISTS(SELECT 1 FROM mame_features WHERE "source_element_id"=225 AND "status" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_features_attribute_positions WHERE "source_element_id"=225 AND field_kind='status' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:feature:status',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_features SET "status"='' WHERE "source_element_id"=225;
INSERT INTO mame_field_assertions VALUES ('optional-invalid:feature:status',(changes()=0));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_features_attribute_positions WHERE "source_element_id"=225 AND field_kind='overall' AND field_occurrence=0;
UPDATE mame_features SET "overall"=NULL WHERE "source_element_id"=225;
INSERT INTO mame_field_assertions VALUES ('optional-absent:feature:overall',(EXISTS(SELECT 1 FROM mame_features WHERE "source_element_id"=225 AND "overall" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_features_attribute_positions WHERE "source_element_id"=225 AND field_kind='overall' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:feature:overall',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_features SET "overall"='' WHERE "source_element_id"=225;
INSERT INTO mame_field_assertions VALUES ('optional-invalid:feature:overall',(changes()=0));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_devices_attribute_positions WHERE "source_element_id"=226 AND field_kind='tag' AND field_occurrence=0;
UPDATE mame_devices SET "tag"=NULL WHERE "source_element_id"=226;
INSERT INTO mame_field_assertions VALUES ('optional-absent:device:tag',(EXISTS(SELECT 1 FROM mame_devices WHERE "source_element_id"=226 AND "tag" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_devices_attribute_positions WHERE "source_element_id"=226 AND field_kind='tag' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:device:tag',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_devices SET "tag"='' WHERE "source_element_id"=226;
INSERT INTO mame_machine_devices_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (226,'tag',0,1,36,22);
INSERT INTO mame_field_assertions VALUES ('optional-empty:device:tag',(EXISTS(SELECT 1 FROM mame_devices WHERE "source_element_id"=226 AND "tag"='') AND EXISTS(SELECT 1 FROM mame_machine_devices_attribute_positions WHERE "source_element_id"=226 AND field_kind='tag' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_devices_attribute_positions WHERE "source_element_id"=226 AND field_kind='fixed_image' AND field_occurrence=0;
UPDATE mame_devices SET "fixed_image"=NULL WHERE "source_element_id"=226;
INSERT INTO mame_field_assertions VALUES ('optional-absent:device:fixed_image',(EXISTS(SELECT 1 FROM mame_devices WHERE "source_element_id"=226 AND "fixed_image" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_devices_attribute_positions WHERE "source_element_id"=226 AND field_kind='fixed_image' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:device:fixed_image',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_devices SET "fixed_image"='' WHERE "source_element_id"=226;
INSERT INTO mame_machine_devices_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (226,'fixed_image',0,2,36,34);
INSERT INTO mame_field_assertions VALUES ('optional-empty:device:fixed_image',(EXISTS(SELECT 1 FROM mame_devices WHERE "source_element_id"=226 AND "fixed_image"='') AND EXISTS(SELECT 1 FROM mame_machine_devices_attribute_positions WHERE "source_element_id"=226 AND field_kind='fixed_image' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_devices_attribute_positions WHERE "source_element_id"=226 AND field_kind='mandatory' AND field_occurrence=0;
UPDATE mame_devices SET "mandatory"=NULL WHERE "source_element_id"=226;
INSERT INTO mame_field_assertions VALUES ('optional-absent:device:mandatory',(EXISTS(SELECT 1 FROM mame_devices WHERE "source_element_id"=226 AND "mandatory" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_devices_attribute_positions WHERE "source_element_id"=226 AND field_kind='mandatory' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:device:mandatory',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_devices SET "mandatory"='' WHERE "source_element_id"=226;
INSERT INTO mame_machine_devices_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (226,'mandatory',0,3,36,46);
INSERT INTO mame_field_assertions VALUES ('optional-empty:device:mandatory',(EXISTS(SELECT 1 FROM mame_devices WHERE "source_element_id"=226 AND "mandatory"='') AND EXISTS(SELECT 1 FROM mame_machine_devices_attribute_positions WHERE "source_element_id"=226 AND field_kind='mandatory' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_devices_attribute_positions WHERE "source_element_id"=226 AND field_kind='interface' AND field_occurrence=0;
UPDATE mame_devices SET "interface"=NULL WHERE "source_element_id"=226;
INSERT INTO mame_field_assertions VALUES ('optional-absent:device:interface',(EXISTS(SELECT 1 FROM mame_devices WHERE "source_element_id"=226 AND "interface" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_devices_attribute_positions WHERE "source_element_id"=226 AND field_kind='interface' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:device:interface',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_devices SET "interface"='' WHERE "source_element_id"=226;
INSERT INTO mame_machine_devices_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (226,'interface',0,4,36,58);
INSERT INTO mame_field_assertions VALUES ('optional-empty:device:interface',(EXISTS(SELECT 1 FROM mame_devices WHERE "source_element_id"=226 AND "interface"='') AND EXISTS(SELECT 1 FROM mame_machine_devices_attribute_positions WHERE "source_element_id"=226 AND field_kind='interface' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_software_lists_attribute_positions WHERE "source_element_id"=231 AND field_kind='filter' AND field_occurrence=0;
UPDATE mame_softwarelist_references SET "filter"=NULL WHERE "source_element_id"=231;
INSERT INTO mame_field_assertions VALUES ('optional-absent:softwarelist:filter',(EXISTS(SELECT 1 FROM mame_softwarelist_references WHERE "source_element_id"=231 AND "filter" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_software_lists_attribute_positions WHERE "source_element_id"=231 AND field_kind='filter' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:softwarelist:filter',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_softwarelist_references SET "filter"='' WHERE "source_element_id"=231;
INSERT INTO mame_machine_software_lists_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (231,'filter',0,3,41,46);
INSERT INTO mame_field_assertions VALUES ('optional-empty:softwarelist:filter',(EXISTS(SELECT 1 FROM mame_softwarelist_references WHERE "source_element_id"=231 AND "filter"='') AND EXISTS(SELECT 1 FROM mame_machine_software_lists_attribute_positions WHERE "source_element_id"=231 AND field_kind='filter' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_machine_ram_options_attribute_positions WHERE "source_element_id"=232 AND field_kind='default' AND field_occurrence=0;
UPDATE mame_ram_options SET "default"=NULL WHERE "source_element_id"=232;
INSERT INTO mame_field_assertions VALUES ('optional-absent:ramoption:default',(EXISTS(SELECT 1 FROM mame_ram_options WHERE "source_element_id"=232 AND "default" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_machine_ram_options_attribute_positions WHERE "source_element_id"=232 AND field_kind='default' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:ramoption:default',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_ram_options SET "default"='' WHERE "source_element_id"=232;
INSERT INTO mame_machine_ram_options_attribute_positions ("source_element_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (232,'default',0,1,42,22);
INSERT INTO mame_field_assertions VALUES ('optional-empty:ramoption:default',(EXISTS(SELECT 1 FROM mame_ram_options WHERE "source_element_id"=232 AND "default"='') AND EXISTS(SELECT 1 FROM mame_machine_ram_options_attribute_positions WHERE "source_element_id"=232 AND field_kind='default' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_row;
DELETE FROM mame_machine_compatibility_attribute_positions WHERE "set_id"=100 AND field_kind='isconsumable' AND field_occurrence=0;
DELETE FROM mame_machine_compatibility WHERE "set_id"=100;
INSERT INTO mame_field_assertions VALUES ('optional-row:compat.machine:isconsumable',(NOT EXISTS(SELECT 1 FROM mame_machine_compatibility WHERE "set_id"=100) AND NOT EXISTS(SELECT 1 FROM mame_machine_compatibility_attribute_positions WHERE "set_id"=100 AND field_kind='isconsumable' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-row-audit:compat.machine',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
ROLLBACK TO optional_row;
RELEASE optional_row;
SAVEPOINT optional_state;
DELETE FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='soundonly' AND field_occurrence=0;
UPDATE mame_rom_compatibility SET "soundonly"=NULL WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('optional-absent:compat.rom:soundonly',(EXISTS(SELECT 1 FROM mame_rom_compatibility WHERE "media_entry_id"=201 AND "soundonly" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='soundonly' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:compat.rom:soundonly',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_rom_compatibility SET "soundonly"=2 WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('optional-invalid:compat.rom:soundonly',(changes()=0));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='dispose' AND field_occurrence=0;
UPDATE mame_rom_compatibility SET "dispose"=NULL WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('optional-absent:compat.rom:dispose',(EXISTS(SELECT 1 FROM mame_rom_compatibility WHERE "media_entry_id"=201 AND "dispose" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='dispose' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:compat.rom:dispose',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_rom_compatibility SET "dispose"=2 WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('optional-invalid:compat.rom:dispose',(changes()=0));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='loadflag' AND field_occurrence=0;
UPDATE mame_rom_compatibility SET "loadflag"=NULL WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('optional-absent:compat.rom:loadflag',(EXISTS(SELECT 1 FROM mame_rom_compatibility WHERE "media_entry_id"=201 AND "loadflag" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='loadflag' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:compat.rom:loadflag',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_rom_compatibility SET "loadflag"='' WHERE "media_entry_id"=201;
INSERT INTO mame_rom_compatibility_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'loadflag',0,13,11,166);
INSERT INTO mame_field_assertions VALUES ('optional-empty:compat.rom:loadflag',(EXISTS(SELECT 1 FROM mame_rom_compatibility WHERE "media_entry_id"=201 AND "loadflag"='') AND EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='loadflag' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='value' AND field_occurrence=0;
UPDATE mame_rom_compatibility SET "value"=NULL WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('optional-absent:compat.rom:value',(EXISTS(SELECT 1 FROM mame_rom_compatibility WHERE "media_entry_id"=201 AND "value" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='value' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:compat.rom:value',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_rom_compatibility SET "value"='' WHERE "media_entry_id"=201;
INSERT INTO mame_rom_compatibility_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'value',0,14,11,178);
INSERT INTO mame_field_assertions VALUES ('optional-empty:compat.rom:value',(EXISTS(SELECT 1 FROM mame_rom_compatibility WHERE "media_entry_id"=201 AND "value"='') AND EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='value' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='inverted' AND field_occurrence=0;
UPDATE mame_rom_compatibility SET "inverted"=NULL WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('optional-absent:compat.rom:inverted',(EXISTS(SELECT 1 FROM mame_rom_compatibility WHERE "media_entry_id"=201 AND "inverted" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='inverted' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:compat.rom:inverted',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_rom_compatibility SET "inverted"=2 WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('optional-invalid:compat.rom:inverted',(changes()=0));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='ovha' AND field_occurrence=0;
UPDATE mame_rom_compatibility SET "ovha"=NULL WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('optional-absent:compat.rom:ovha',(EXISTS(SELECT 1 FROM mame_rom_compatibility WHERE "media_entry_id"=201 AND "ovha" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='ovha' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:compat.rom:ovha',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_rom_compatibility SET "ovha"='' WHERE "media_entry_id"=201;
INSERT INTO mame_rom_compatibility_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column") VALUES (201,'ovha',0,16,11,202);
INSERT INTO mame_field_assertions VALUES ('optional-empty:compat.rom:ovha',(EXISTS(SELECT 1 FROM mame_rom_compatibility WHERE "media_entry_id"=201 AND "ovha"='') AND EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='ovha' AND field_occurrence=0)));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_state;
DELETE FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='nothread' AND field_occurrence=0;
UPDATE mame_rom_compatibility SET "nothread"=NULL WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('optional-absent:compat.rom:nothread',(EXISTS(SELECT 1 FROM mame_rom_compatibility WHERE "media_entry_id"=201 AND "nothread" IS NULL) AND NOT EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='nothread' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-absence-audit:compat.rom:nothread',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE OR IGNORE mame_rom_compatibility SET "nothread"=2 WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('optional-invalid:compat.rom:nothread',(changes()=0));
ROLLBACK TO optional_state;
RELEASE optional_state;
SAVEPOINT optional_row;
DELETE FROM mame_disk_compatibility_attribute_positions WHERE "media_entry_id"=202 AND field_kind='writeable' AND field_occurrence=0;
DELETE FROM mame_disk_compatibility WHERE "media_entry_id"=202;
INSERT INTO mame_field_assertions VALUES ('optional-row:compat.disk:writeable',(NOT EXISTS(SELECT 1 FROM mame_disk_compatibility WHERE "media_entry_id"=202) AND NOT EXISTS(SELECT 1 FROM mame_disk_compatibility_attribute_positions WHERE "media_entry_id"=202 AND field_kind='writeable' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('optional-row-audit:compat.disk',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
ROLLBACK TO optional_row;
RELEASE optional_row;

-- Sparse relationship literals retain empty target text and absent rows.
SAVEPOINT optional_link;
DELETE FROM mame_machines_attribute_positions WHERE "set_id"=100 AND field_kind='cloneof' AND field_occurrence=0;
DELETE FROM mame_machine_links WHERE machine_id=100 AND link_kind='cloneof';
INSERT INTO mame_field_assertions VALUES ('relationship-absent:machine:cloneof',(NOT EXISTS(SELECT 1 FROM mame_machine_links WHERE machine_id=100 AND link_kind='cloneof') AND NOT EXISTS(SELECT 1 FROM mame_machines_attribute_positions WHERE "set_id"=100 AND field_kind='cloneof' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('relationship-absence-audit:machine:cloneof',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
ROLLBACK TO optional_link;
RELEASE optional_link;
SAVEPOINT optional_link;
DELETE FROM mame_machines_attribute_positions WHERE "set_id"=100 AND field_kind='romof' AND field_occurrence=0;
DELETE FROM mame_machine_links WHERE machine_id=100 AND link_kind='romof';
INSERT INTO mame_field_assertions VALUES ('relationship-absent:machine:romof',(NOT EXISTS(SELECT 1 FROM mame_machine_links WHERE machine_id=100 AND link_kind='romof') AND NOT EXISTS(SELECT 1 FROM mame_machines_attribute_positions WHERE "set_id"=100 AND field_kind='romof' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('relationship-absence-audit:machine:romof',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
ROLLBACK TO optional_link;
RELEASE optional_link;
SAVEPOINT optional_link;
DELETE FROM mame_machines_attribute_positions WHERE "set_id"=100 AND field_kind='sampleof' AND field_occurrence=0;
DELETE FROM mame_machine_links WHERE machine_id=100 AND link_kind='sampleof';
INSERT INTO mame_field_assertions VALUES ('relationship-absent:machine:sampleof',(NOT EXISTS(SELECT 1 FROM mame_machine_links WHERE machine_id=100 AND link_kind='sampleof') AND NOT EXISTS(SELECT 1 FROM mame_machines_attribute_positions WHERE "set_id"=100 AND field_kind='sampleof' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('relationship-absence-audit:machine:sampleof',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
ROLLBACK TO optional_link;
RELEASE optional_link;
SAVEPOINT optional_link;
DELETE FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='merge' AND field_occurrence=0;
DELETE FROM mame_rom_merges WHERE "media_entry_id"=201;
INSERT INTO mame_field_assertions VALUES ('relationship-absent:rom:merge',(NOT EXISTS(SELECT 1 FROM mame_rom_merges WHERE "media_entry_id"=201) AND NOT EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='merge' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('relationship-absence-audit:rom:merge',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
ROLLBACK TO optional_link;
RELEASE optional_link;
SAVEPOINT optional_link;
DELETE FROM mame_disk_claims_attribute_positions WHERE "media_entry_id"=202 AND field_kind='merge' AND field_occurrence=0;
DELETE FROM mame_disk_merges WHERE "media_entry_id"=202;
INSERT INTO mame_field_assertions VALUES ('relationship-absent:disk:merge',(NOT EXISTS(SELECT 1 FROM mame_disk_merges WHERE "media_entry_id"=202) AND NOT EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions WHERE "media_entry_id"=202 AND field_kind='merge' AND field_occurrence=0)));
INSERT INTO mame_field_assertions VALUES ('relationship-absence-audit:disk:merge',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
ROLLBACK TO optional_link;
RELEASE optional_link;

-- Hash absence, empty, invalid and value states for all four source fields.
SAVEPOINT hash_state;
DELETE FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='crc' AND field_occurrence=0;
DELETE FROM catalog_entry_hashes WHERE reported_hash_id=501;
INSERT INTO mame_field_assertions VALUES ('hash-absent:rom:crc',(NOT EXISTS(SELECT 1 FROM catalog_entry_hashes WHERE reported_hash_id=501) AND NOT EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='crc' AND field_occurrence=0)));
INSERT INTO catalog_entry_hashes ("reported_hash_id","media_entry_id","source_hash_field","field_occurrence","presence","hash_scope","hash_id","reported_text") VALUES (501,201,'crc',0,'empty','whole_file',NULL,NULL);
INSERT INTO mame_rom_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column","reported_hash_id") VALUES (201,'crc',0,3,11,46,501);
INSERT INTO mame_field_assertions VALUES ('hash-empty:rom:crc',(SELECT presence='empty' AND hash_id IS NULL AND reported_text IS NULL AND declared_text='' FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE reported_hash_id=501));
UPDATE catalog_entry_hashes SET presence='invalid',reported_text='invalid-hex' WHERE reported_hash_id=501;
INSERT INTO invalid_catalog_entry_hashes ("reported_hash_id","diagnostic_code") VALUES (501,'constructed_invalid_hex');
INSERT INTO mame_field_assertions VALUES ('hash-invalid:rom:crc',(SELECT presence='invalid' AND hash_id IS NULL AND declared_text='invalid-hex' FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) JOIN invalid_catalog_entry_hashes USING(reported_hash_id) WHERE reported_hash_id=501));
DELETE FROM invalid_catalog_entry_hashes WHERE reported_hash_id=501;
UPDATE catalog_entry_hashes SET presence='value',hash_id=1,reported_text='DEADBEEF' WHERE reported_hash_id=501;
INSERT INTO mame_field_assertions VALUES ('hash-override:rom:crc',(SELECT presence='value' AND hash_id=1 AND declared_text='DEADBEEF' FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE reported_hash_id=501));
INSERT INTO mame_field_assertions VALUES ('hash-state-audit:rom:crc',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
ROLLBACK TO hash_state;
RELEASE hash_state;
SAVEPOINT hash_state;
DELETE FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='sha1' AND field_occurrence=0;
DELETE FROM catalog_entry_hashes WHERE reported_hash_id=502;
INSERT INTO mame_field_assertions VALUES ('hash-absent:rom:sha1',(NOT EXISTS(SELECT 1 FROM catalog_entry_hashes WHERE reported_hash_id=502) AND NOT EXISTS(SELECT 1 FROM mame_rom_claims_attribute_positions WHERE "media_entry_id"=201 AND field_kind='sha1' AND field_occurrence=0)));
INSERT INTO catalog_entry_hashes ("reported_hash_id","media_entry_id","source_hash_field","field_occurrence","presence","hash_scope","hash_id","reported_text") VALUES (502,201,'sha1',0,'empty','whole_file',NULL,NULL);
INSERT INTO mame_rom_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column","reported_hash_id") VALUES (201,'sha1',0,4,11,58,502);
INSERT INTO mame_field_assertions VALUES ('hash-empty:rom:sha1',(SELECT presence='empty' AND hash_id IS NULL AND reported_text IS NULL AND declared_text='' FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE reported_hash_id=502));
UPDATE catalog_entry_hashes SET presence='invalid',reported_text='invalid-hex' WHERE reported_hash_id=502;
INSERT INTO invalid_catalog_entry_hashes ("reported_hash_id","diagnostic_code") VALUES (502,'constructed_invalid_hex');
INSERT INTO mame_field_assertions VALUES ('hash-invalid:rom:sha1',(SELECT presence='invalid' AND hash_id IS NULL AND declared_text='invalid-hex' FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) JOIN invalid_catalog_entry_hashes USING(reported_hash_id) WHERE reported_hash_id=502));
DELETE FROM invalid_catalog_entry_hashes WHERE reported_hash_id=502;
UPDATE catalog_entry_hashes SET presence='value',hash_id=2,reported_text='0123456789ABCDEF0123456789ABCDEF01234567' WHERE reported_hash_id=502;
INSERT INTO mame_field_assertions VALUES ('hash-override:rom:sha1',(SELECT presence='value' AND hash_id=2 AND declared_text='0123456789ABCDEF0123456789ABCDEF01234567' FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE reported_hash_id=502));
INSERT INTO mame_field_assertions VALUES ('hash-state-audit:rom:sha1',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
ROLLBACK TO hash_state;
RELEASE hash_state;
SAVEPOINT hash_state;
DELETE FROM mame_disk_claims_attribute_positions WHERE "media_entry_id"=202 AND field_kind='sha1' AND field_occurrence=0;
DELETE FROM catalog_entry_hashes WHERE reported_hash_id=504;
INSERT INTO mame_field_assertions VALUES ('hash-absent:disk:sha1',(NOT EXISTS(SELECT 1 FROM catalog_entry_hashes WHERE reported_hash_id=504) AND NOT EXISTS(SELECT 1 FROM mame_disk_claims_attribute_positions WHERE "media_entry_id"=202 AND field_kind='sha1' AND field_occurrence=0)));
INSERT INTO catalog_entry_hashes ("reported_hash_id","media_entry_id","source_hash_field","field_occurrence","presence","hash_scope","hash_id","reported_text") VALUES (504,202,'sha1',0,'empty','chd_header_sha1',NULL,NULL);
INSERT INTO mame_disk_claims_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column","reported_hash_id") VALUES (202,'sha1',0,1,12,22,504);
INSERT INTO mame_field_assertions VALUES ('hash-empty:disk:sha1',(SELECT presence='empty' AND hash_id IS NULL AND reported_text IS NULL AND declared_text='' FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE reported_hash_id=504));
UPDATE catalog_entry_hashes SET presence='invalid',reported_text='invalid-hex' WHERE reported_hash_id=504;
INSERT INTO invalid_catalog_entry_hashes ("reported_hash_id","diagnostic_code") VALUES (504,'constructed_invalid_hex');
INSERT INTO mame_field_assertions VALUES ('hash-invalid:disk:sha1',(SELECT presence='invalid' AND hash_id IS NULL AND declared_text='invalid-hex' FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) JOIN invalid_catalog_entry_hashes USING(reported_hash_id) WHERE reported_hash_id=504));
DELETE FROM invalid_catalog_entry_hashes WHERE reported_hash_id=504;
UPDATE catalog_entry_hashes SET presence='value',hash_id=2,reported_text='0123456789ABCDEF0123456789ABCDEF01234567' WHERE reported_hash_id=504;
INSERT INTO mame_field_assertions VALUES ('hash-override:disk:sha1',(SELECT presence='value' AND hash_id=2 AND declared_text='0123456789ABCDEF0123456789ABCDEF01234567' FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE reported_hash_id=504));
INSERT INTO mame_field_assertions VALUES ('hash-state-audit:disk:sha1',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
ROLLBACK TO hash_state;
RELEASE hash_state;
SAVEPOINT hash_state;
DELETE FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='md5' AND field_occurrence=0;
DELETE FROM catalog_entry_hashes WHERE reported_hash_id=503;
INSERT INTO mame_field_assertions VALUES ('hash-absent:compat.rom:md5',(NOT EXISTS(SELECT 1 FROM catalog_entry_hashes WHERE reported_hash_id=503) AND NOT EXISTS(SELECT 1 FROM mame_rom_compatibility_attribute_positions WHERE "media_entry_id"=201 AND field_kind='md5' AND field_occurrence=0)));
INSERT INTO catalog_entry_hashes ("reported_hash_id","media_entry_id","source_hash_field","field_occurrence","presence","hash_scope","hash_id","reported_text") VALUES (503,201,'md5',0,'empty','unknown',NULL,NULL);
INSERT INTO mame_rom_compatibility_attribute_positions ("media_entry_id","field_kind","field_occurrence","source_order","source_line","source_column","reported_hash_id") VALUES (201,'md5',0,10,11,130,503);
INSERT INTO mame_field_assertions VALUES ('hash-empty:compat.rom:md5',(SELECT presence='empty' AND hash_id IS NULL AND reported_text IS NULL AND declared_text='' FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE reported_hash_id=503));
UPDATE catalog_entry_hashes SET presence='invalid',reported_text='invalid-hex' WHERE reported_hash_id=503;
INSERT INTO invalid_catalog_entry_hashes ("reported_hash_id","diagnostic_code") VALUES (503,'constructed_invalid_hex');
INSERT INTO mame_field_assertions VALUES ('hash-invalid:compat.rom:md5',(SELECT presence='invalid' AND hash_id IS NULL AND declared_text='invalid-hex' FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) JOIN invalid_catalog_entry_hashes USING(reported_hash_id) WHERE reported_hash_id=503));
DELETE FROM invalid_catalog_entry_hashes WHERE reported_hash_id=503;
UPDATE catalog_entry_hashes SET presence='value',hash_id=3,reported_text='0123456789ABCDEF0123456789ABCDEF' WHERE reported_hash_id=503;
INSERT INTO mame_field_assertions VALUES ('hash-override:compat.rom:md5',(SELECT presence='value' AND hash_id=3 AND declared_text='0123456789ABCDEF0123456789ABCDEF' FROM catalog_entry_hashes JOIN declared_catalog_hash_text USING(reported_hash_id) WHERE reported_hash_id=503));
INSERT INTO mame_field_assertions VALUES ('hash-state-audit:compat.rom:md5',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
ROLLBACK TO hash_state;
RELEASE hash_state;

-- Optional text children have no row when absent, and empty text is present.
UPDATE OR IGNORE mame_machine_text_elements SET "text_value"=NULL WHERE source_element_id=233;
INSERT INTO mame_field_assertions VALUES ('text-notnull:description',(changes()=0));
UPDATE OR IGNORE mame_machine_text_elements SET "text_value"=NULL WHERE source_element_id=234;
INSERT INTO mame_field_assertions VALUES ('text-notnull:year',(changes()=0));
SAVEPOINT text_absence;
DELETE FROM mame_machine_text_elements WHERE source_element_id=234;
INSERT INTO mame_field_assertions VALUES ('text-absent:year',(NOT EXISTS(SELECT 1 FROM mame_machine_text_elements WHERE source_element_id=234)));
ROLLBACK TO text_absence;
RELEASE text_absence;
UPDATE OR IGNORE mame_machine_text_elements SET "text_value"=NULL WHERE source_element_id=235;
INSERT INTO mame_field_assertions VALUES ('text-notnull:manufacturer',(changes()=0));
SAVEPOINT text_absence;
DELETE FROM mame_machine_text_elements WHERE source_element_id=235;
INSERT INTO mame_field_assertions VALUES ('text-absent:manufacturer',(NOT EXISTS(SELECT 1 FROM mame_machine_text_elements WHERE source_element_id=235)));
ROLLBACK TO text_absence;
RELEASE text_absence;
UPDATE OR IGNORE mame_ram_options SET "text"=NULL WHERE source_element_id=232;
INSERT INTO mame_field_assertions VALUES ('text-notnull:ramoption.text',(changes()=0));

-- Position corruption attacks are detected by the TEMP typed presence view.
SAVEPOINT corrupt_position;
DELETE FROM mame_machine_chips_attribute_positions WHERE source_element_id=205 AND field_kind='clock';
INSERT INTO mame_field_assertions VALUES ('attack:position-deleted',(EXISTS(SELECT 1 FROM mame_fixture_presence_problems WHERE owner_id=205 AND problem='mame_machine_chips_attribute_positions/clock')));
ROLLBACK TO corrupt_position;
RELEASE corrupt_position;
SAVEPOINT corrupt_value;
UPDATE mame_chips SET clock=NULL WHERE source_element_id=205;
INSERT INTO mame_field_assertions VALUES ('attack:absent-value-present-position',(EXISTS(SELECT 1 FROM mame_fixture_presence_problems WHERE owner_id=205 AND problem='mame_machine_chips_attribute_positions/clock')));
ROLLBACK TO corrupt_value;
RELEASE corrupt_value;
SAVEPOINT invented_default;
UPDATE mame_displays SET flipx_specified=0 WHERE source_element_id=206;
INSERT INTO mame_field_assertions VALUES ('attack:invented-default-position',(EXISTS(SELECT 1 FROM mame_fixture_presence_problems WHERE owner_id=206 AND problem='mame_machine_displays_attribute_positions/flipx')));
ROLLBACK TO invented_default;
RELEASE invented_default;
SAVEPOINT equal_count_move;
INSERT INTO catalog_source_elements VALUES (236,1,'mame_chip');
INSERT INTO mame_chips VALUES (236,100,'second',NULL,'cpu',NULL,22,46,3);
INSERT INTO mame_machine_chips_attribute_positions (source_element_id,field_kind,source_order,source_line,source_column) VALUES (236,'name',1,46,10),(236,'type',2,46,25);
INSERT INTO mame_field_assertions VALUES ('move-baseline:valid',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
UPDATE mame_machine_chips_attribute_positions SET source_element_id=236,source_order=0 WHERE source_element_id=205 AND field_kind='clock';
INSERT INTO mame_field_assertions VALUES ('attack:equal-count-move',(SELECT count(*)=2 FROM mame_fixture_presence_problems WHERE problem='mame_machine_chips_attribute_positions/clock' AND owner_id IN (205,236)));
ROLLBACK TO equal_count_move;
RELEASE equal_count_move;

-- Deliberate proof boundary: deleting BOTH an optional source value and its
-- position satisfies retained-fact closure. Only independent parser evidence
-- can distinguish this from authentic omission; do not claim detection here.
SAVEPOINT erased_optional;
UPDATE mame_chips SET clock=NULL WHERE source_element_id=205;
DELETE FROM mame_machine_chips_attribute_positions WHERE source_element_id=205 AND field_kind='clock';
INSERT INTO mame_field_assertions VALUES ('boundary:joint-optional-erasure',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
ROLLBACK TO erased_optional;
RELEASE erased_optional;

INSERT INTO mame_field_assertions VALUES ('final:presence',(SELECT count(*)=0 FROM mame_fixture_presence_problems));
INSERT INTO mame_field_assertions VALUES ('final:foreign-keys',(SELECT count(*)=0 FROM pragma_foreign_key_check));
INSERT INTO mame_field_assertions VALUES ('final:reading-rule',(SELECT count(*)=0 FROM candidate_mame_integrity_problems));
.eqp on
SELECT source_element_id FROM mame_displays WHERE machine_id=100 ORDER BY source_order;
.eqp off
SELECT 'MAME independent field witnesses passed' AS result;
