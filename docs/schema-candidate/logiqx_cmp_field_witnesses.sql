-- DESIGN ONLY, independent of logiqx_cmp_witnesses.sql and the coverage TSV.
-- Prerequisites: repository root; fresh empty SQLite >= 3.37 with CLI .read;
-- sqlite3 :memory: < docs/schema-candidate/logiqx_cmp_field_witnesses.sql
-- Minimal common-key/hash fixtures deliberately do NOT implement shared.sql,
-- relationships.sql, parser/normalization, edition closure or publication.
-- Constructed source coordinates/extents are synthetic, not capture evidence.
.bail on
PRAGMA foreign_keys = ON;

CREATE TABLE catalog_reading_rules (
    reading_rules_id INTEGER PRIMARY KEY,
    format_family TEXT NOT NULL,
    rules_version TEXT NOT NULL
) STRICT;
CREATE TABLE catalog_editions (
    edition_id INTEGER PRIMARY KEY,
    reading_rules_id INTEGER NOT NULL REFERENCES catalog_reading_rules(reading_rules_id)
) STRICT;
CREATE TABLE catalog_set_groups (
    set_group_id INTEGER PRIMARY KEY,
    edition_id INTEGER NOT NULL REFERENCES catalog_editions(edition_id),
    group_kind TEXT NOT NULL,
    UNIQUE (set_group_id, edition_id)
) STRICT;
CREATE TABLE catalog_source_elements (
    source_element_id INTEGER PRIMARY KEY,
    edition_id INTEGER NOT NULL REFERENCES catalog_editions(edition_id),
    element_kind TEXT NOT NULL,
    UNIQUE (source_element_id, edition_id)
) STRICT;
CREATE TABLE catalog_sets (
    set_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_group_id INTEGER NOT NULL REFERENCES catalog_set_groups(set_group_id),
    set_name TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_group_id, source_order)
) STRICT;
CREATE TABLE catalog_media_entries (
    media_entry_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    file_uuid BLOB
) STRICT;
CREATE TABLE hash_values (
    hash_id INTEGER PRIMARY KEY,
    algorithm TEXT NOT NULL CHECK (algorithm IN ('crc32', 'md5', 'sha1', 'sha256')),
    bytes BLOB NOT NULL,
    CHECK (length(bytes) = CASE algorithm WHEN 'crc32' THEN 4 WHEN 'md5' THEN 16 WHEN 'sha1' THEN 20 WHEN 'sha256' THEN 32 END),
    UNIQUE (algorithm, bytes)
) STRICT;
CREATE TABLE catalog_entry_hashes (
    reported_hash_id INTEGER PRIMARY KEY,
    media_entry_id INTEGER NOT NULL REFERENCES catalog_media_entries(media_entry_id),
    source_hash_field TEXT NOT NULL CHECK (source_hash_field IN ('crc', 'crc32', 'md5', 'sha1', 'sha256', 'origin_sha256')),
    field_occurrence INTEGER NOT NULL CHECK (field_occurrence >= 0),
    presence TEXT NOT NULL CHECK (presence IN ('empty', 'invalid', 'value')),
    hash_scope TEXT NOT NULL CHECK (hash_scope IN ('whole_file', 'whole_asset', 'unknown', 'chd_header_sha1', 'source_origin', 'nfo_companion')),
    hash_id INTEGER REFERENCES hash_values(hash_id),
    reported_text TEXT,
    CHECK ((presence = 'empty' AND hash_id IS NULL AND reported_text IS NULL) OR
           (presence = 'invalid' AND hash_id IS NULL AND reported_text IS NOT NULL AND reported_text <> '') OR
           (presence = 'value' AND hash_id IS NOT NULL)),
    UNIQUE (media_entry_id, source_hash_field, field_occurrence)
) STRICT;
CREATE TABLE reported_catalog_relationships (relationship_id INTEGER PRIMARY KEY) STRICT;


.read docs/schema-candidate/logiqx_cmp.sql

INSERT INTO catalog_reading_rules VALUES
 (1,'logiqx','logiqx-declared-text-compat-v2'),
 (2,'clrmamepro','clrmamepro-declared-text-compat-v1');
INSERT INTO catalog_editions VALUES (1,1),(2,2);
INSERT INTO catalog_set_groups VALUES (10,1,'root'),(20,2,'root');
INSERT INTO logiqx_documents(edition_id,set_group_id,build,debug,debug_was_present,extent_view,extent_start,extent_end)
 VALUES(1,10,'','no',1,'retained_original_bytes',0,10000);
INSERT INTO clrmamepro_documents(edition_id,set_group_id,header_present,comment_count,extent_view,extent_start,extent_end)
 VALUES(2,20,1,1,'retained_original_bytes',0,10000);
INSERT INTO catalog_source_elements VALUES
 (101,1,'logiqx_header'),(102,1,'logiqx_clrmamepro_options'),(103,1,'logiqx_romcenter_options'),
 (104,1,'logiqx_game'),(105,1,'logiqx_release'),(106,1,'logiqx_bios_set'),
 (107,1,'logiqx_rom'),(108,1,'logiqx_disk'),(109,1,'logiqx_sample'),
 (110,1,'logiqx_archive_reference'),(111,1,'logiqx_device_reference'),
 (112,1,'logiqx_game_comment'),(113,1,'logiqx_root_file_name'),(114,1,'logiqx_root_sha1'),
 (201,2,'clrmamepro_header'),(202,2,'clrmamepro_set'),(203,2,'clrmamepro_rom'),
 (204,2,'clrmamepro_sample'),(205,2,'clrmamepro_comment');
WITH kinds(k) AS(VALUES(0),(1),(2),(3),(4),(5),(6),(7),(8),(9))
 INSERT INTO catalog_source_elements SELECT 120+k,1,'logiqx_header_text' FROM kinds;
WITH kinds(k) AS(VALUES(0),(1),(2))
 INSERT INTO catalog_source_elements SELECT 140+k,1,'logiqx_game_text' FROM kinds;
INSERT INTO hash_values VALUES(1,'crc32',zeroblob(4)),(2,'md5',zeroblob(16)),(3,'sha1',zeroblob(20));
INSERT INTO reported_catalog_relationships VALUES(1),(2),(3),(4),(5),(6),(7),(8),(9);
INSERT INTO logiqx_headers VALUES(101,10,2,3,1);
INSERT INTO logiqx_root_file_names VALUES(113,10,'',0,1,1);
INSERT INTO logiqx_root_sha1_elements VALUES(114,10,3,'unknown',1,2,1);
INSERT INTO logiqx_clrmamepro_options VALUES(102,101,10,14,1,'','split',1,'obsolete',1,'zip',1);
INSERT INTO logiqx_romcenter_options VALUES(103,101,11,15,1,'','split',1,'split',1,'merged',1,'no',1,'no',1,'no',1);
WITH kinds(k) AS(VALUES(0),(1),(2),(3),(4),(5),(6),(7),(8),(9))
 INSERT INTO logiqx_header_text_elements SELECT 120+k,101,k,'',k,4+k,1 FROM kinds;
INSERT INTO catalog_sets VALUES(104,10,'',3,16,1),(202,20,'',1,20,1);
INSERT INTO logiqx_games VALUES(104,'','no',1,'','');
INSERT INTO logiqx_set_links VALUES(104,'cloneof','',1),(104,'romof','',2),(104,'sampleof','',3);
WITH kinds(k) AS(VALUES(0),(1),(2))
 INSERT INTO logiqx_game_text_elements SELECT 140+k,104,k,'',k,17+k,1 FROM kinds;
INSERT INTO logiqx_game_comments VALUES(112,104,'',3,20,1);
INSERT INTO logiqx_releases VALUES(105,104,'','','','','no',1,4,21,1);
INSERT INTO logiqx_bios_sets VALUES(106,104,'','','no',1,5,22,1);
INSERT INTO catalog_media_entries VALUES(107,NULL),(108,NULL),(109,NULL),(203,NULL),(204,NULL);
INSERT INTO logiqx_roms(media_entry_id,set_id,name,size_text,status,status_specified,date,source_order,source_line,source_column)
 VALUES(107,104,'','','good',1,'',6,23,1);
INSERT INTO logiqx_rom_compatibility VALUES(107,'');
INSERT INTO logiqx_rom_merges VALUES(107,'',4);
INSERT INTO logiqx_disks VALUES(108,104,'','good',1,7,24,1);
INSERT INTO logiqx_disk_merges VALUES(108,'',5);
INSERT INTO logiqx_samples VALUES(109,104,'',8,25,1);
INSERT INTO logiqx_archive_references VALUES(110,104,'',9,26,1);
INSERT INTO logiqx_device_references VALUES(111,104,'',6,10,27,1);
-- Explicit, independent policy codes; no lookup from the ledger or DDL.
INSERT INTO logiqx_document_attribute_positions(edition_id,field_kind,source_order,source_line,source_column)
 VALUES(1,0,0,1,10),(1,1,1,1,20);
WITH codes(k) AS(VALUES(0),(1),(2),(3))
 INSERT INTO logiqx_clrmamepro_option_positions(source_element_id,field_kind,source_order,source_line,source_column)
 SELECT 102,k,k,14,10+k FROM codes;
WITH codes(k) AS(VALUES(0),(1),(2),(3),(4),(5),(6))
 INSERT INTO logiqx_romcenter_option_positions(source_element_id,field_kind,source_order,source_line,source_column)
 SELECT 103,k,k,15,10+k FROM codes;
WITH codes(k,rel) AS(VALUES(0,NULL),(1,NULL),(2,NULL),(3,1),(4,2),(5,3),(6,NULL),(7,NULL))
 INSERT INTO logiqx_game_attribute_positions(set_id,field_kind,source_order,source_line,source_column,relationship_id)
 SELECT 104,k,k,16,10+k,rel FROM codes;
WITH codes(k) AS(VALUES(0),(1),(2),(3),(4))
 INSERT INTO logiqx_release_attribute_positions(source_element_id,field_kind,source_order,source_line,source_column)
 SELECT 105,k,k,21,10+k FROM codes;
WITH codes(k) AS(VALUES(0),(1),(2))
 INSERT INTO logiqx_bios_attribute_positions(source_element_id,field_kind,source_order,source_line,source_column)
 SELECT 106,k,k,22,10+k FROM codes;
INSERT INTO logiqx_archive_attribute_positions(source_element_id,field_kind,source_order,source_line,source_column)
 VALUES(110,0,0,26,10);
INSERT INTO logiqx_device_reference_attribute_positions(source_element_id,field_kind,source_order,source_line,source_column,relationship_id)
 VALUES(111,0,0,27,10,6);
INSERT INTO logiqx_sample_attribute_positions(media_entry_id,field_kind,source_order,source_line,source_column)
 VALUES(109,0,0,25,10);
INSERT INTO catalog_entry_hashes VALUES
 (11,107,'crc',0,'value','unknown',1,NULL),
 (12,107,'sha1',0,'value','unknown',3,NULL),
 (13,107,'md5',0,'value','unknown',2,NULL),
 (14,108,'sha1',0,'value','unknown',3,NULL),
 (15,108,'md5',0,'value','unknown',2,NULL);
WITH codes(k,h,rel) AS(VALUES(0,NULL,NULL),(1,NULL,NULL),(2,11,NULL),(3,12,NULL),(4,13,NULL),
 (5,NULL,4),(6,NULL,NULL),(7,NULL,NULL),(8,NULL,NULL))
 INSERT INTO logiqx_rom_attribute_positions(media_entry_id,field_kind,source_order,source_line,source_column,reported_hash_id,relationship_id)
 SELECT 107,k,k,23,10+k,h,rel FROM codes;
WITH codes(k,h,rel) AS(VALUES(0,NULL,NULL),(1,14,NULL),(2,15,NULL),(3,NULL,5),(4,NULL,NULL))
 INSERT INTO logiqx_disk_attribute_positions(media_entry_id,field_kind,source_order,source_line,source_column,reported_hash_id,relationship_id)
 SELECT 108,k,k,24,10+k,h,rel FROM codes;

INSERT INTO clrmamepro_headers VALUES(201,2,'ClrMamePro',0,1,1,'','','','','','','','','','');
UPDATE clrmamepro_header_options SET header_definition='',forcemerging='split',
 forcezipping='zip',forcepacking='',forcenodump='obsolete' WHERE header_id=201;
INSERT INTO clrmamepro_sets VALUES(202,'SeT','','','','','','','','','');
INSERT INTO clrmamepro_set_links VALUES(202,'cloneof','',7),(202,'sampleof','',8);
INSERT INTO clrmamepro_roms(media_entry_id,set_id,name,size_text,source_order,source_line,source_column)
 VALUES(203,202,'','0008',12,40,1);
UPDATE clrmamepro_rom_details SET date='',serial='',status_text='' WHERE media_entry_id=203;
INSERT INTO clrmamepro_rom_merges VALUES(203,'',9);
INSERT INTO clrmamepro_samples VALUES(204,202,'','SaMpLe',13,60,1,60,8,1);
INSERT INTO clrmamepro_comments VALUES(205,2,';',61,1);
WITH fields(k,w) AS(VALUES(0,'name'),(1,'description'),(2,'version'),(3,'date'),(4,'author'),
 (5,'email'),(6,'homepage'),(7,'url'),(8,'comment'),(9,'category'),(10,'header'),
 (11,'forcemerging'),(12,'forcezipping'),(13,'forcepacking'),(14,'forcenodump'))
 INSERT INTO clrmamepro_header_field_positions(header_id,field_kind,source_order,keyword,keyword_line,keyword_column,value_line,value_column,value_is_quoted)
 SELECT 201,k,k,upper(w),2+k,1,2+k,20,1 FROM fields;
WITH fields(k,w,rel) AS(VALUES(0,'name',NULL),(1,'cloneof',7),(2,'description',NULL),(3,'year',NULL),
 (4,'manufacturer',NULL),(5,'rebuildto',NULL),(6,'sampleof',8),(7,'region',NULL),
 (8,'releaseyear',NULL),(9,'releasemonth',NULL),(10,'releaseday',NULL),(11,'serial',NULL))
 INSERT INTO clrmamepro_set_field_positions(set_id,field_kind,source_order,keyword,keyword_line,keyword_column,value_line,value_column,value_is_quoted,relationship_id)
 SELECT 202,k,k,upper(w),21+k,1,21+k,20,1,rel FROM fields;
INSERT INTO catalog_entry_hashes VALUES
 (21,203,'crc',0,'value','whole_asset',1,NULL),(22,203,'crc32',0,'value','whole_asset',1,NULL),
 (23,203,'md5',0,'value','whole_asset',2,NULL),(24,203,'sha1',0,'value','whole_asset',3,NULL);
WITH fields(k,w,h,rel) AS(VALUES(0,'name',NULL,NULL),(1,'size',NULL,NULL),(2,'crc',21,NULL),
 (3,'crc32',22,NULL),(4,'md5',23,NULL),(5,'sha1',24,NULL),(6,'merge',NULL,9),
 (7,'date',NULL,NULL),(8,'serial',NULL,NULL),(9,'status',NULL,NULL),
 (10,'nodump',NULL,NULL),(11,'baddump',NULL,NULL))
 INSERT INTO clrmamepro_rom_field_positions(media_entry_id,field_kind,source_order,keyword,keyword_line,keyword_column,value_line,value_column,value_is_quoted,reported_hash_id,relationship_id)
 SELECT 203,k,k,upper(w),41+k,1,CASE WHEN k<10 THEN 41+k END,CASE WHEN k<10 THEN 20 END,
 k<10,h,rel FROM fields;

CREATE TEMP TABLE field_assert(label TEXT PRIMARY KEY,ok INTEGER NOT NULL CHECK(ok=1)) STRICT;
INSERT INTO field_assert VALUES('fully populated native field/position agreement',
 NOT EXISTS(SELECT 1 FROM candidate_logiqx_cmp_integrity_problems));
INSERT INTO field_assert SELECT 'required empty game/ROM/release/BIOS/sample names retained',
 (SELECT set_name='' FROM catalog_sets WHERE set_id=104)
 AND (SELECT name='' FROM logiqx_roms WHERE media_entry_id=107)
 AND (SELECT name='' AND region='' FROM logiqx_releases WHERE source_element_id=105)
 AND (SELECT name='' AND description='' FROM logiqx_bios_sets WHERE source_element_id=106)
 AND (SELECT name='' FROM logiqx_samples WHERE media_entry_id=109);
INSERT INTO field_assert SELECT 'text children retain empty direct text',
 (SELECT count(*)=10 AND min(text_value='') FROM logiqx_header_text_elements)
 AND (SELECT count(*)=3 AND min(text_value='') FROM logiqx_game_text_elements);
INSERT INTO field_assert SELECT 'separate root text and nonmedia SHA1',
 (SELECT file_name='' FROM logiqx_root_file_names)
 AND (SELECT length(h.bytes)=20 AND h.algorithm='sha1' FROM logiqx_root_sha1_elements r JOIN hash_values h USING(hash_id));
INSERT INTO field_assert SELECT 'CMP raw lexical facts and leading zeros',
 (SELECT source_block='SeT' FROM clrmamepro_sets)
 AND (SELECT sample_name='' AND source_field='SaMpLe' FROM clrmamepro_samples)
 AND (SELECT size_text='0008' AND size_value=8 FROM clrmamepro_roms);
INSERT INTO field_assert SELECT 'CMP hash and relationship anchors only on Value',
 (SELECT count(*)=4 AND min(role='Value') FROM clrmamepro_position_tokens WHERE reported_hash_id IS NOT NULL)
 AND (SELECT count(*)=3 AND min(role='Value') FROM clrmamepro_position_tokens WHERE relationship_id IS NOT NULL);
-- These row-selected assertions always emit one evaluated aggregate row;
-- zero matches and multiple matches fail instead of skipping the assertion.
INSERT INTO field_assert SELECT 'quoted empty CMP header scalar has both lexical anchors',
 count(*)=1 AND coalesce(min(h.name='' AND p.value_is_quoted=1 AND p.keyword_line>0 AND p.value_line>0),0)=1
 FROM clrmamepro_headers h JOIN clrmamepro_header_field_positions p USING(header_id) WHERE p.field_kind=0;
SAVEPOINT unknown_directives;
UPDATE clrmamepro_header_options SET forcemerging='unknown',forcezipping='',forcenodump='unknown';
INSERT INTO field_assert SELECT 'unknown and empty CMP directives retain text without effective modes',
 count(*)=1 AND coalesce(min(forcemerging='unknown' AND forcemerging_effective IS NULL AND forcezipping=''
 AND forcezipping_effective IS NULL AND forcenodump='unknown' AND forcenodump_effective IS NULL),0)=1
 FROM clrmamepro_header_options WHERE header_id=201;
ROLLBACK TO unknown_directives; RELEASE unknown_directives;
-- The 15 XML defaults begin explicitly supplied, with all their positions.
INSERT INTO field_assert SELECT 'explicit default values retain all presence bits',
 (SELECT debug_was_present=1 AND debug='no' FROM logiqx_documents)
 AND (SELECT isbios_was_present=1 AND isbios='no' FROM logiqx_games)
 AND (SELECT default_was_present=1 AND "default"='no' FROM logiqx_releases)
 AND (SELECT default_was_present=1 AND is_default='no' FROM logiqx_bios_sets)
 AND (SELECT status_specified=1 AND status='good' FROM logiqx_roms)
 AND (SELECT status_specified=1 AND status='good' FROM logiqx_disks)
 AND (SELECT forcemerging_was_present=1 AND forcenodump_was_present=1 AND forcepacking_was_present=1
       AND forcemerging='split' AND forcenodump='obsolete' AND forcepacking='zip' FROM logiqx_clrmamepro_options)
 AND (SELECT rommode_was_present=1 AND biosmode_was_present=1 AND samplemode_was_present=1
       AND lockrommode_was_present=1 AND lockbiosmode_was_present=1 AND locksamplemode_was_present=1
       AND rommode='split' AND biosmode='split' AND samplemode='merged'
       AND lockrommode='no' AND lockbiosmode='no' AND locksamplemode='no' FROM logiqx_romcenter_options);

SAVEPOINT omitted_defaults;
UPDATE logiqx_documents SET debug_was_present=0;
UPDATE logiqx_games SET isbios_was_present=0;
UPDATE logiqx_releases SET default_was_present=0;
UPDATE logiqx_bios_sets SET default_was_present=0;
UPDATE logiqx_roms SET status_specified=0;
UPDATE logiqx_disks SET status_specified=0;
UPDATE logiqx_clrmamepro_options SET forcemerging_was_present=0,forcenodump_was_present=0,forcepacking_was_present=0;
UPDATE logiqx_romcenter_options SET rommode_was_present=0,biosmode_was_present=0,samplemode_was_present=0,
 lockrommode_was_present=0,lockbiosmode_was_present=0,locksamplemode_was_present=0;
DELETE FROM logiqx_document_attribute_positions WHERE field_kind=1;
DELETE FROM logiqx_game_attribute_positions WHERE field_kind=2;
DELETE FROM logiqx_release_attribute_positions WHERE field_kind=4;
DELETE FROM logiqx_bios_attribute_positions WHERE field_kind=2;
DELETE FROM logiqx_rom_attribute_positions WHERE field_kind=6;
DELETE FROM logiqx_disk_attribute_positions WHERE field_kind=4;
DELETE FROM logiqx_clrmamepro_option_positions WHERE field_kind>0;
DELETE FROM logiqx_romcenter_option_positions WHERE field_kind>0;
INSERT INTO field_assert VALUES('omitted defaults have no positions but retain effective defaults',
 NOT EXISTS(SELECT 1 FROM candidate_logiqx_cmp_integrity_problems));
ROLLBACK TO omitted_defaults; RELEASE omitted_defaults;

SAVEPOINT optional_text;
UPDATE logiqx_documents SET build=NULL;
DELETE FROM logiqx_document_attribute_positions WHERE field_kind=0;
UPDATE logiqx_clrmamepro_options SET header=NULL;
DELETE FROM logiqx_clrmamepro_option_positions WHERE field_kind=0;
UPDATE logiqx_romcenter_options SET plugin=NULL;
DELETE FROM logiqx_romcenter_option_positions WHERE field_kind=0;
UPDATE logiqx_games SET sourcefile=NULL,board=NULL,rebuildto=NULL;
DELETE FROM logiqx_game_attribute_positions WHERE field_kind IN(1,6,7);
UPDATE logiqx_releases SET language=NULL,date=NULL;
DELETE FROM logiqx_release_attribute_positions WHERE field_kind IN(2,3);
UPDATE logiqx_roms SET size_text=NULL,date=NULL;
DELETE FROM logiqx_rom_attribute_positions WHERE field_kind IN(1,7,8);
DELETE FROM logiqx_rom_compatibility;
DELETE FROM logiqx_header_text_elements WHERE field_kind<>0;
DELETE FROM logiqx_game_text_elements;
DELETE FROM logiqx_root_file_names;
DELETE FROM logiqx_root_sha1_elements;
DELETE FROM logiqx_game_comments;
UPDATE clrmamepro_headers SET name=NULL,description=NULL,version=NULL,date=NULL,author=NULL,
 email=NULL,homepage=NULL,url=NULL,comment=NULL,category=NULL;
UPDATE clrmamepro_header_options SET header_definition=NULL,forcemerging=NULL,forcezipping=NULL,
 forcepacking=NULL,forcenodump=NULL;
DELETE FROM clrmamepro_header_field_positions;
UPDATE clrmamepro_sets SET description=NULL,year=NULL,manufacturer=NULL,rebuildto=NULL,region=NULL,
 releaseyear=NULL,releasemonth=NULL,releaseday=NULL,serial=NULL;
DELETE FROM clrmamepro_set_field_positions WHERE field_kind NOT IN(0,1,6);
UPDATE clrmamepro_roms SET size_text=NULL;
UPDATE clrmamepro_rom_details SET date=NULL,serial=NULL,status_text=NULL;
DELETE FROM clrmamepro_rom_field_positions WHERE field_kind IN(1,7,8,9,10,11);
DELETE FROM logiqx_game_attribute_positions WHERE field_kind IN(3,4,5);
DELETE FROM logiqx_set_links;
DELETE FROM logiqx_rom_attribute_positions WHERE field_kind IN(2,3,4,5);
DELETE FROM logiqx_rom_merges;
DELETE FROM logiqx_disk_attribute_positions WHERE field_kind IN(1,2,3);
DELETE FROM logiqx_disk_merges;
DELETE FROM clrmamepro_set_field_positions WHERE field_kind IN(1,6);
DELETE FROM clrmamepro_set_links;
DELETE FROM clrmamepro_rom_field_positions WHERE field_kind IN(2,3,4,5,6);
DELETE FROM clrmamepro_rom_merges;
DELETE FROM catalog_entry_hashes;
INSERT INTO field_assert VALUES('optional NULL text has no positions; compatible description/size omittable',
 NOT EXISTS(SELECT 1 FROM candidate_logiqx_cmp_integrity_problems));
INSERT INTO field_assert SELECT 'present CMP header omitted forcenodump derives obsolete',
 count(*)=1 AND coalesce(min(forcenodump IS NULL AND forcenodump_effective='obsolete'),0)=1
 FROM clrmamepro_header_options WHERE header_id=201;
ROLLBACK TO optional_text; RELEASE optional_text;

SAVEPOINT required_presence;
DELETE FROM logiqx_header_text_elements WHERE field_kind=0;
INSERT INTO field_assert VALUES('missing compatible header name diagnosed',
 EXISTS(SELECT 1 FROM candidate_logiqx_cmp_integrity_problems WHERE problem='missing_required_logiqx_header_name'));
ROLLBACK TO required_presence; RELEASE required_presence;
SAVEPOINT explicit_default_position;
DELETE FROM logiqx_romcenter_option_positions WHERE field_kind=6;
INSERT INTO field_assert VALUES('explicit default requires position',
 EXISTS(SELECT 1 FROM candidate_logiqx_cmp_integrity_problems WHERE problem='missing_position' AND owner_id=103));
ROLLBACK TO explicit_default_position; RELEASE explicit_default_position;
SAVEPOINT hash_states;
UPDATE catalog_entry_hashes SET presence='empty',hash_id=NULL WHERE reported_hash_id=11;
UPDATE catalog_entry_hashes SET presence='invalid',hash_id=NULL,reported_text='not-hex' WHERE reported_hash_id=12;
INSERT INTO field_assert VALUES('Logiqx supplied empty and invalid declarations keep canonical positions',
 NOT EXISTS(SELECT 1 FROM candidate_logiqx_cmp_integrity_problems));
INSERT INTO field_assert SELECT 'Logiqx hash states are distinct',
 (SELECT presence='empty' AND hash_id IS NULL FROM catalog_entry_hashes WHERE reported_hash_id=11)
 AND (SELECT presence='invalid' AND reported_text='not-hex' FROM catalog_entry_hashes WHERE reported_hash_id=12);
ROLLBACK TO hash_states; RELEASE hash_states;
SAVEPOINT cmp_hash_states;
UPDATE catalog_entry_hashes SET presence='empty',hash_id=NULL WHERE reported_hash_id=21;
UPDATE catalog_entry_hashes SET presence='invalid',hash_id=NULL,reported_text='bad' WHERE reported_hash_id=22;
INSERT INTO field_assert VALUES('CMP empty/invalid hashes reach exact family rejection report',
 (SELECT count(*)=2 FROM candidate_logiqx_cmp_integrity_problems
  WHERE problem='cmp_invalid_hash_state' AND edition_id=2 AND owner_id IN(21,22)));
ROLLBACK TO cmp_hash_states; RELEASE cmp_hash_states;
INSERT INTO field_assert VALUES('fixture restored; no native integrity or FK problems',
 NOT EXISTS(SELECT 1 FROM candidate_logiqx_cmp_integrity_problems)
 AND NOT EXISTS(SELECT 1 FROM pragma_foreign_key_check));
SELECT 'Logiqx/CMP independent field witness: passed';
