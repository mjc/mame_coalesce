-- Bounded SQLite-only No-Intro fixture. Run after the assembled candidate DDL
-- with foreign_keys=ON. It exercises all three interpretations, hash-position
-- links, mixed native ownership, NFO scope separation, and known/missing extents.
-- Execute with no_intro_field_check.py: -- expect-error directives are checked
-- by its statement runner. sqlite3.executescript alone cannot expect failures.
SAVEPOINT no_intro_candidate_witness;

CREATE TEMP TABLE no_intro_witness_assertions (
    assertion TEXT PRIMARY KEY,
    passed INTEGER NOT NULL CHECK (passed = 1)
) STRICT, WITHOUT ROWID;

INSERT INTO catalog_publishers VALUES (1,'witness-publisher','Witness',NULL);
INSERT INTO catalogs VALUES
    (10,1,'witness-dat','DAT'), (11,1,'witness-pc','PC fixture'), (12,1,'witness-export','Export');
INSERT INTO catalog_source_files(source_file_id,sha256,byte_length,object_key,codec) VALUES
    (20,randomblob(32),100,'witness/dat.xml.zst','zstd'),
    (21,randomblob(32),100,'witness/pc.xml.zst','zstd'),
    (22,randomblob(32),100,'witness/export-nested.xml.zst','zstd'),
    (23,randomblob(32),100,'witness/export-sibling.xml.zst','zstd');
INSERT INTO catalog_reading_rules VALUES
    (30,'witness-dat','no_intro_dat','no-intro-dat-v4-compatible','v4','reader','compatible'),
    (31,'witness-pc','no_intro_pc_fixture','synthetic','fixture','reader','fixture'),
    (32,'witness-export','no_intro_database','observed','274','reader','compatible');
INSERT INTO catalog_coverage VALUES (40,'unknown');
INSERT INTO catalog_editions(edition_id,catalog_id,source_file_id,reading_rules_id,coverage_id) VALUES
    (50,10,20,30,40),(51,11,21,31,40),(52,12,22,32,40),(53,12,23,32,40);
INSERT INTO catalog_set_groups VALUES (60,50,'root'),(61,51,'root'),(62,52,'root'),(63,53,'root');

INSERT INTO catalog_source_elements(source_element_id,edition_id,element_kind) VALUES
    (100,50,'no_intro_dat_header'),(101,50,'no_intro_dat_header_text_child'),
    (106,50,'no_intro_dat_header_text_child'),(107,50,'no_intro_dat_header_text_child'),(108,50,'no_intro_dat_header_text_child'),
    (102,50,'no_intro_dat_game'),(103,50,'no_intro_dat_game_description'),
    (104,50,'no_intro_dat_category'),(105,50,'no_intro_dat_rom'),
    (200,51,'no_intro_pc_header'),(201,51,'no_intro_pc_header_name'),
    (202,51,'no_intro_pc_header_version'),(203,51,'no_intro_pc_game'),
    (204,51,'no_intro_pc_game_description'),(205,51,'no_intro_pc_rom'),(206,51,'no_intro_pc_game'),
    (300,52,'no_intro_export_nested_header'),(301,52,'no_intro_export_header_field'),
    (302,52,'no_intro_export_game'),(303,52,'no_intro_export_archive'),
    (304,52,'no_intro_export_dump_source'),(305,52,'no_intro_export_dump_details'),
    (306,52,'no_intro_export_source_file'),(307,52,'no_intro_export_release'),
    (308,52,'no_intro_export_release_details'),(309,52,'no_intro_export_release_file');

INSERT INTO catalog_sets(set_id,set_group_id,set_name,source_order,source_line,source_column) VALUES
    (102,60,'dat-game',1,2,8),(203,61,'pc-game',1,2,7),(206,61,'pc-clone-child',2,2,75),(302,62,'export-game',1,3,8);
INSERT INTO catalog_media_entries(media_entry_id,file_uuid) VALUES (105,NULL),(205,NULL),(306,NULL),(309,NULL);
INSERT INTO hash_values(hash_id,algorithm,bytes) VALUES
    (800,'crc32',x'01020304'),(801,'sha1',zeroblob(20)),
    (802,'sha256',zeroblob(32)),(803,'sha256',x'0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20');
INSERT INTO catalog_relationships(relationship_id,assertion_key,origin,edition_id) VALUES
    (920,'witness:dat:cloneof','source',50),(921,'witness:dat:cloneofid','source',50),
    (922,'witness:pc:clone','source',51),(923,'witness:pc:mergeof','source',51),
    (924,'witness:archive:mergeof','source',52);
INSERT INTO reported_catalog_relationships(relationship_id,reported_kind) VALUES
    (920,'no_intro_dat_cloneof'),(921,'no_intro_dat_cloneofid'),
    (922,'no_intro_pc_clone'),(923,'no_intro_pc_mergeof'),
    (924,'no_intro_database_archive_mergeof');
INSERT INTO catalog_entry_hashes(reported_hash_id,media_entry_id,source_hash_field,field_occurrence,presence,hash_scope,hash_id) VALUES
    (900,105,'crc',0,'value','unknown',800),
    (901,205,'sha1',0,'value','whole_asset',801),
    (902,306,'origin_sha256',0,'value','source_origin',802),
    (903,309,'sha256',0,'value','unknown',803);

INSERT INTO no_intro_dat_documents(edition_id,location_view,start_line,start_column,column_convention)
VALUES (50,'transport_decoded_xml_text',1,1,'one_based_unicode_scalar');
INSERT INTO no_intro_dat_headers VALUES (100,60,0,1,1);
INSERT INTO no_intro_dat_header_text_children VALUES
    (101,100,'name','DAT header',1,1,9),
    (106,100,'id','witness-dat',0,1,4),
    (107,100,'description','Witness DAT',2,1,20),
    (108,100,'version','1',3,1,33);
INSERT INTO no_intro_dat_games VALUES (102,NULL);
INSERT INTO no_intro_dat_set_links VALUES
    (102,'cloneof','parent-set',920),(102,'cloneofid','parent-number',921);
INSERT INTO no_intro_dat_game_descriptions VALUES (103,102,'Description',0,2,1);
INSERT INTO no_intro_dat_categories VALUES (104,102,'Arcade',1,2,20);
INSERT INTO no_intro_dat_rom_claims(media_entry_id,set_id,name,size_text,source_order,source_line,source_column)
VALUES (105,102,'game.rom','12',2,2,35);
INSERT INTO no_intro_dat_game_field_positions VALUES
    (102,'name',0,0,2,8,NULL),(102,'cloneof',0,1,2,18,920),(102,'cloneofid',0,2,2,41,921);
INSERT INTO no_intro_dat_rom_field_positions VALUES
    (105,'name',0,0,2,42,NULL),(105,'crc',0,1,2,55,900),
    (105,'size',0,2,2,54,NULL);

INSERT INTO no_intro_pc_documents(edition_id,location_view,start_line,start_column,column_convention)
VALUES (51,'transport_decoded_xml_text',1,1,'one_based_unicode_scalar');
INSERT INTO no_intro_pc_headers VALUES (200,61,0,1,9);
INSERT INTO no_intro_pc_header_names VALUES (201,200,'Name',0,1,17);
INSERT INTO no_intro_pc_header_versions VALUES (202,200,'1',1,1,24);
INSERT INTO no_intro_pc_games VALUES (203,'0007',NULL,NULL,NULL,NULL);
INSERT INTO no_intro_pc_games VALUES (206,'0008',NULL,NULL,NULL,NULL);
INSERT INTO no_intro_pc_clone_markers VALUES (203,'P');
INSERT INTO no_intro_pc_clone_links VALUES (206,'7',922);
INSERT INTO no_intro_pc_merge_links VALUES (203,'4',923);
INSERT INTO no_intro_pc_game_descriptions VALUES (204,203,'fixture description',0,2,1);
INSERT INTO no_intro_pc_languages VALUES (203,0,'English');
INSERT INTO no_intro_pc_file_claims(media_entry_id,set_id,name,size_text,source_order,source_line,source_column)
VALUES (205,203,'game.bin','18446744073709551615',1,2,28);
INSERT INTO no_intro_pc_game_attribute_positions VALUES
    (203,'name',0,0,2,7,NULL),(203,'languages',0,1,2,20,NULL),
    (203,'clone',0,2,2,31,NULL),(203,'mergeof',0,3,2,39,923),
    (206,'name',0,0,2,75,NULL),(206,'clone',0,1,2,83,922);
INSERT INTO no_intro_pc_rom_attribute_positions VALUES
    (205,'name',0,0,2,33,NULL),(205,'sha1',0,1,2,45,901);

INSERT INTO no_intro_export_documents(edition_id,root_set_group_id,envelope_mode,header_present,extent_view,extent_start,extent_end,
    location_view,start_line,start_column,end_line,end_column,column_convention)
VALUES (52,62,'single_datafile',1,NULL,NULL,NULL,'transport_decoded_xml_text',1,1,NULL,NULL,'one_based_unicode_scalar');
INSERT INTO no_intro_export_datafiles(edition_id,location_view,start_line,start_column,column_convention)
VALUES (52,'transport_decoded_xml_text',1,12,'one_based_unicode_scalar');
INSERT INTO no_intro_export_headers(header_id,edition_id,location_view,start_line,start_column,end_line,end_column,column_convention)
VALUES (700,52,'transport_decoded_xml_text',2,1,2,10,'one_based_unicode_scalar');
INSERT INTO no_intro_export_header_placements VALUES (300,700,52,0);
INSERT INTO no_intro_export_header_fields VALUES (301,700,'author','',0,2,12,2,21);
INSERT INTO no_intro_export_games VALUES (302,NULL,NULL,NULL);
INSERT INTO no_intro_export_game_field_positions VALUES (302,'name',0,0,3,8);
INSERT INTO no_intro_archive_descriptions(archive_id,set_id,name,source_order,source_line,source_column,source_end_line,source_end_column)
VALUES (303,302,'variant',0,3,20,3,40);
INSERT INTO no_intro_archive_clone_markers VALUES (303,'P');
INSERT INTO no_intro_archive_merge_links VALUES (303,'2',924);
INSERT INTO no_intro_archive_field_positions VALUES
    (303,'name',0,0,3,28,NULL),(303,'clone',0,1,3,30,NULL),(303,'mergeof',0,2,3,34,924);
INSERT INTO no_intro_dump_sources VALUES (304,302,1,3,41,3,90);
INSERT INTO no_intro_dump_details(details_element_id,dump_source_id,id,source_order,source_line,source_column,source_end_line,source_end_column,opening_end_line,opening_end_column)
VALUES (305,304,'publisher-1',0,3,50,3,70,3,69);
INSERT INTO no_intro_dump_details_field_positions VALUES (305,'id',0,0,3,58);
INSERT INTO no_intro_dump_files(media_entry_id,dump_source_id,id,size_text,source_order,source_line,source_column,source_end_line,source_end_column)
VALUES (306,304,'1','bad-size',1,3,71,3,89);
INSERT INTO no_intro_dump_file_field_positions VALUES
    (306,'id',0,0,3,78,NULL),(306,'origin_sha256',0,1,3,82,902);
INSERT INTO no_intro_releases VALUES (307,302,2,3,91,3,140);
INSERT INTO no_intro_release_details(details_element_id,release_id,nfo_size,nfosize,source_order,source_line,source_column,source_end_line,source_end_column,opening_end_line,opening_end_column)
VALUES (308,307,'10','11',0,3,100,3,120,3,119);
INSERT INTO no_intro_release_nfo_hashes VALUES (910,308,'nfocrc',0,'bad-nfo','invalid',NULL);
INSERT INTO invalid_no_intro_release_nfo_hashes VALUES (910,'invalid_hash');
INSERT INTO no_intro_release_details_field_positions VALUES (308,'nfo_size',0,0,3,108,NULL),(308,'nfocrc',0,1,3,113,910);
INSERT INTO no_intro_release_files(media_entry_id,release_id,id,size_text,source_order,source_line,source_column,source_end_line,source_end_column)
VALUES (309,307,'1','12',2,3,121,3,139);
INSERT INTO no_intro_release_file_field_positions VALUES (309,'sha256',0,0,3,130,903);
INSERT INTO no_intro_export_parse_witnesses VALUES (52,1,1,1,1,0,1,1,1,0,1,1,3,1,0,2,2,0,1);
INSERT INTO no_intro_export_game_witnesses VALUES (302,1,1,1);
INSERT INTO no_intro_dump_source_witnesses VALUES (304,1,0,1);
INSERT INTO no_intro_release_witnesses VALUES (307,1,0,1);

-- Sibling-root header: same canonical owner, with no source-element placement.
INSERT INTO no_intro_export_documents(edition_id,root_set_group_id,envelope_mode,header_present,location_view,start_line,start_column,column_convention)
VALUES (53,63,'sibling_header_datafile',1,'transport_decoded_xml_text',1,1,'one_based_unicode_scalar');
INSERT INTO no_intro_export_datafiles(edition_id,location_view,start_line,start_column,column_convention)
VALUES (53,'transport_decoded_xml_text',1,30,'one_based_unicode_scalar');
INSERT INTO no_intro_export_headers(header_id,edition_id,location_view,start_line,start_column,end_line,end_column,column_convention)
VALUES (701,53,'transport_decoded_xml_text',1,1,1,20,'one_based_unicode_scalar');
INSERT INTO no_intro_export_parse_witnesses VALUES (53,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0);

INSERT INTO no_intro_witness_assertions
SELECT 'three families retain one native game and one media declaration each',
       (SELECT count(*)=1 FROM no_intro_dat_rom_claims)
       AND (SELECT count(*)=1 FROM no_intro_pc_file_claims)
       AND (SELECT count(*)=1 FROM no_intro_dump_files)
       AND (SELECT count(*)=1 FROM no_intro_release_files);
INSERT INTO no_intro_witness_assertions
SELECT 'export game name order and location exist only on canonical name position',
       (SELECT count(*)=0 FROM pragma_table_info('no_intro_export_games')
        WHERE name IN ('name_source_order','name_source_line','name_source_column'))
       AND (SELECT field_kind='name' AND source_order=0 AND source_line=3 AND source_column=8
            FROM no_intro_export_game_field_positions WHERE set_id=302);
INSERT INTO no_intro_witness_assertions
SELECT 'hash positions identify their exact common declaration',
       (SELECT count(*)=4 FROM catalog_entry_hashes)
       AND (SELECT count(*)=4 FROM (
           SELECT reported_hash_id FROM no_intro_dat_rom_field_positions WHERE reported_hash_id IS NOT NULL
           UNION ALL SELECT reported_hash_id FROM no_intro_pc_rom_attribute_positions WHERE reported_hash_id IS NOT NULL
           UNION ALL SELECT reported_hash_id FROM no_intro_dump_file_field_positions WHERE reported_hash_id IS NOT NULL
           UNION ALL SELECT reported_hash_id FROM no_intro_release_file_field_positions WHERE reported_hash_id IS NOT NULL));
INSERT INTO no_intro_witness_assertions
SELECT 'NFO hash remains on release details and is absent from media hashes',
       (SELECT presence='invalid' AND hash_id IS NULL FROM no_intro_release_nfo_hashes WHERE reported_nfo_hash_id=910)
       AND (SELECT count(*)=1 FROM invalid_no_intro_release_nfo_hashes WHERE reported_nfo_hash_id=910)
       AND (SELECT count(*)=0 FROM catalog_entry_hashes WHERE source_hash_field IN ('nfo_crc32','nfocrc'));
INSERT INTO no_intro_witness_assertions
SELECT 'relationship positions reuse the exact typed declaration identities; P clone markers stay NULL',
       (SELECT count(*)=2 FROM no_intro_dat_game_field_positions WHERE relationship_id IS NOT NULL)
       AND (SELECT count(*)=2 FROM no_intro_pc_game_attribute_positions WHERE relationship_id IS NOT NULL)
       AND (SELECT relationship_id IS NULL FROM no_intro_pc_game_attribute_positions
            WHERE source_element_id=203 AND field_kind='clone')
       AND (SELECT relationship_id IS NULL FROM no_intro_archive_field_positions
            WHERE archive_id=303 AND field_kind='clone')
       AND (SELECT count(*)=0 FROM candidate_no_intro_integrity_problems
            WHERE problem LIKE '%relationship_position_mismatch');
-- expect-error: relationship position requires its exact typed declaration
UPDATE no_intro_pc_game_attribute_positions SET relationship_id=920
WHERE source_element_id=203 AND field_kind='clone';
SAVEPOINT pc_position_audit;
DROP TRIGGER candidate_relationship_position_no_intro_pc_game_attribute_positions_update;
UPDATE no_intro_pc_game_attribute_positions SET relationship_id=920
WHERE source_element_id=203 AND field_kind='clone';
INSERT INTO no_intro_witness_assertions
SELECT 'P clone marker rejects a position relationship identity from another declaration',
       EXISTS(SELECT 1 FROM candidate_no_intro_integrity_problems
              WHERE problem='pc_relationship_position_mismatch' AND owner_id=203);
ROLLBACK TO pc_position_audit;
RELEASE pc_position_audit;
-- expect-error: CHECK constraint failed
UPDATE no_intro_dat_game_field_positions SET relationship_id=NULL
WHERE set_id=102 AND field_kind='cloneof';
INSERT INTO no_intro_witness_assertions
SELECT 'DAT clone positions cannot omit their canonical relationship identity',
       NOT EXISTS(SELECT 1 FROM no_intro_dat_game_field_positions
                  WHERE set_id=102 AND field_kind='cloneof' AND relationship_id IS NULL);
-- expect-error: relationship position requires its exact typed declaration
UPDATE no_intro_dat_game_field_positions SET relationship_id=922
WHERE set_id=102 AND field_kind='cloneof';
SAVEPOINT dat_position_audit;
DROP TRIGGER candidate_relationship_position_no_intro_dat_game_field_positions_update;
UPDATE no_intro_dat_game_field_positions SET relationship_id=922
WHERE set_id=102 AND field_kind='cloneof';
INSERT INTO no_intro_witness_assertions
SELECT 'DAT clone position mismatch is surfaced against its typed declaration',
       EXISTS(SELECT 1 FROM candidate_no_intro_integrity_problems
              WHERE problem='dat_game_relationship_position_mismatch' AND owner_id=102);
ROLLBACK TO dat_position_audit;
RELEASE dat_position_audit;
-- expect-error: relationship position requires its exact typed declaration
UPDATE no_intro_pc_game_attribute_positions SET relationship_id=920
WHERE source_element_id=203 AND field_kind='name';
INSERT INTO no_intro_witness_assertions
SELECT 'non-relationship position rejects a relationship identity at table level',
       (SELECT relationship_id IS NULL FROM no_intro_pc_game_attribute_positions
        WHERE source_element_id=203 AND field_kind='name');
INSERT INTO no_intro_witness_assertions
SELECT 'nested header has a placement and canonical identity; unknown root ends stay null',
       (SELECT count(*)=1 FROM no_intro_export_header_placements WHERE header_id=700)
       AND (SELECT extent_start IS NULL AND extent_end IS NULL AND end_line IS NULL
            FROM no_intro_export_documents WHERE edition_id=52)
       AND (SELECT start_line=1 AND end_line IS NULL AND extent_start IS NULL
            FROM no_intro_export_datafiles WHERE edition_id=52);
INSERT INTO no_intro_witness_assertions
SELECT 'sibling-root header has one canonical owner and no placement identity',
       (SELECT envelope_mode='sibling_header_datafile' FROM no_intro_export_documents WHERE edition_id=53)
       AND (SELECT count(*)=1 FROM no_intro_export_headers WHERE edition_id=53)
       AND (SELECT count(*)=0 FROM no_intro_export_header_placements WHERE datafile_edition_id=53);
UPDATE no_intro_export_documents SET envelope_mode='sibling_header_datafile' WHERE edition_id=52;
INSERT INTO no_intro_witness_assertions
SELECT 'single-datafile mode rejects a missing nested header placement',
       EXISTS(SELECT 1 FROM candidate_no_intro_integrity_problems
              WHERE problem='export_header_mode' AND edition_id=52);
UPDATE no_intro_export_documents SET envelope_mode='single_datafile' WHERE edition_id=52;
UPDATE no_intro_export_documents SET header_present=0 WHERE edition_id=52;
INSERT INTO no_intro_witness_assertions
SELECT 'independent header presence detects deletion even with no header fields',
       EXISTS(SELECT 1 FROM candidate_no_intro_integrity_problems
              WHERE problem='export_header_mode' AND edition_id=52);
UPDATE no_intro_export_documents SET header_present=1 WHERE edition_id=52;
UPDATE OR IGNORE no_intro_dat_documents SET column_convention=NULL WHERE edition_id=50;
UPDATE OR IGNORE no_intro_pc_documents SET column_convention=NULL WHERE edition_id=51;
UPDATE OR IGNORE no_intro_export_documents SET column_convention=NULL WHERE edition_id IN (52,53);
UPDATE OR IGNORE no_intro_export_datafiles SET column_convention=NULL WHERE edition_id IN (52,53);
UPDATE OR IGNORE no_intro_export_headers SET column_convention=NULL WHERE header_id IN (700,701);
INSERT INTO no_intro_witness_assertions
SELECT 'all populated physical roots reject NULL coordinate convention',
       (SELECT count(*)=0 FROM (
          SELECT 1 FROM no_intro_dat_documents WHERE location_view IS NOT NULL AND column_convention IS NULL
          UNION ALL SELECT 1 FROM no_intro_pc_documents WHERE location_view IS NOT NULL AND column_convention IS NULL
          UNION ALL SELECT 1 FROM no_intro_export_documents WHERE location_view IS NOT NULL AND column_convention IS NULL
          UNION ALL SELECT 1 FROM no_intro_export_datafiles WHERE location_view IS NOT NULL AND column_convention IS NULL
          UNION ALL SELECT 1 FROM no_intro_export_headers WHERE location_view IS NOT NULL AND column_convention IS NULL));
INSERT INTO no_intro_witness_assertions
SELECT 'observed unsigned-64 P/C size remains source text beyond signed range',
       (SELECT size_text='18446744073709551615' AND size_i64 IS NULL FROM no_intro_pc_file_claims WHERE media_entry_id=205);
INSERT INTO no_intro_witness_assertions
SELECT 'both export envelope modes close all edition count totals',
       (SELECT count(*)=0 FROM candidate_no_intro_integrity_problems);

UPDATE no_intro_export_parse_witnesses SET game_count=game_count+1 WHERE edition_id=52;
INSERT INTO no_intro_witness_assertions
SELECT 'edition count seal detects a parser/count mismatch',
       EXISTS(SELECT 1 FROM candidate_no_intro_integrity_problems
              WHERE problem='export_count_witness_missing_or_mismatch' AND edition_id=52);
UPDATE no_intro_export_parse_witnesses SET game_count=game_count-1 WHERE edition_id=52;

UPDATE no_intro_export_headers SET extent_view='transport_decoded_xml_bytes',extent_start=90,extent_end=110
WHERE header_id=700;
UPDATE no_intro_export_datafiles SET extent_view='transport_decoded_xml_bytes',extent_start=0,extent_end=100
WHERE edition_id=52;
INSERT INTO no_intro_witness_assertions
SELECT 'nested header interval outside its actual datafile is reported',
       EXISTS(SELECT 1 FROM candidate_no_intro_integrity_problems
              WHERE problem='export_header_datafile_extent_containment' AND owner_id=700);
UPDATE no_intro_export_headers SET extent_view=NULL,extent_start=NULL,extent_end=NULL WHERE header_id=700;
UPDATE no_intro_export_datafiles SET extent_view=NULL,extent_start=NULL,extent_end=NULL WHERE edition_id=52;

ANALYZE;
EXPLAIN QUERY PLAN SELECT archive_id FROM no_intro_archive_descriptions
WHERE set_id=302 ORDER BY source_order,archive_id;
EXPLAIN QUERY PLAN SELECT media_entry_id FROM no_intro_dump_files
WHERE dump_source_id=304 ORDER BY source_order,media_entry_id;
EXPLAIN QUERY PLAN SELECT field_kind FROM no_intro_release_details_field_positions
WHERE details_element_id=308 ORDER BY source_order;

SELECT assertion FROM no_intro_witness_assertions ORDER BY assertion;
DROP TABLE no_intro_witness_assertions;
ROLLBACK TO no_intro_candidate_witness;
RELEASE no_intro_candidate_witness;
