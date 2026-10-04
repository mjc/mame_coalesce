-- Position-only provenance for declared Logiqx attributes. Values remain in
-- their fixed native owners and source ordinals remain owner-local.
CREATE TABLE logiqx_document_attribute_positions (
    snapshot_key TEXT NOT NULL CHECK(typeof(snapshot_key)='text') REFERENCES logiqx_document_facts(snapshot_key) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind)='integer' AND field_kind IN (0,1)),
    source_order INTEGER NOT NULL CHECK (typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(snapshot_key,field_kind), UNIQUE(snapshot_key,source_order)
) WITHOUT ROWID;
CREATE TABLE logiqx_clrmamepro_attribute_positions (
    snapshot_key TEXT NOT NULL CHECK(typeof(snapshot_key)='text') REFERENCES logiqx_clrmamepro_options(snapshot_key) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 3),
    source_order INTEGER NOT NULL CHECK (typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(snapshot_key,field_kind), UNIQUE(snapshot_key,source_order)
) WITHOUT ROWID;
CREATE TABLE logiqx_romcenter_attribute_positions (
    snapshot_key TEXT NOT NULL CHECK(typeof(snapshot_key)='text') REFERENCES logiqx_romcenter_options(snapshot_key) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 6),
    source_order INTEGER NOT NULL CHECK (typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(snapshot_key,field_kind), UNIQUE(snapshot_key,source_order)
) WITHOUT ROWID;
CREATE TABLE logiqx_game_attribute_positions (
    set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0) REFERENCES logiqx_games(set_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 7),
    source_order INTEGER NOT NULL CHECK (typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(set_id,field_kind), UNIQUE(set_id,source_order)
) WITHOUT ROWID;
CREATE TABLE logiqx_release_attribute_positions (
    set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0),
    release_order INTEGER NOT NULL CHECK(typeof(release_order)='integer' AND release_order>=0),
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 4),
    source_order INTEGER NOT NULL CHECK (typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(set_id,release_order,field_kind), UNIQUE(set_id,release_order,source_order),
    FOREIGN KEY(set_id,release_order) REFERENCES logiqx_releases(set_id,release_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE logiqx_bios_attribute_positions (
    set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0),
    bios_order INTEGER NOT NULL CHECK(typeof(bios_order)='integer' AND bios_order>=0),
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 2),
    source_order INTEGER NOT NULL CHECK (typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(set_id,bios_order,field_kind), UNIQUE(set_id,bios_order,source_order),
    FOREIGN KEY(set_id,bios_order) REFERENCES logiqx_bios_sets(set_id,bios_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE logiqx_rom_attribute_positions (
    occurrence_id INTEGER NOT NULL CHECK(typeof(occurrence_id)='integer' AND occurrence_id>0) REFERENCES logiqx_rom_claims(occurrence_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 8),
    source_order INTEGER NOT NULL CHECK (typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(occurrence_id,field_kind), UNIQUE(occurrence_id,source_order)
) WITHOUT ROWID;
CREATE TABLE logiqx_disk_attribute_positions (
    occurrence_id INTEGER NOT NULL CHECK(typeof(occurrence_id)='integer' AND occurrence_id>0) REFERENCES logiqx_disk_claims(occurrence_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 4),
    source_order INTEGER NOT NULL CHECK (typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(occurrence_id,field_kind), UNIQUE(occurrence_id,source_order)
) WITHOUT ROWID;
CREATE TABLE logiqx_sample_attribute_positions (
    occurrence_id INTEGER NOT NULL CHECK(typeof(occurrence_id)='integer' AND occurrence_id>0) REFERENCES logiqx_sample_claims(occurrence_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind)='integer' AND field_kind=0),
    source_order INTEGER NOT NULL CHECK (typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(occurrence_id,field_kind), UNIQUE(occurrence_id,source_order)
) WITHOUT ROWID;
CREATE TABLE logiqx_archive_attribute_positions (
    set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0),
    archive_order INTEGER NOT NULL CHECK(typeof(archive_order)='integer' AND archive_order>=0),
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind)='integer' AND field_kind=0),
    source_order INTEGER NOT NULL CHECK (typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(set_id,archive_order,field_kind), UNIQUE(set_id,archive_order,source_order),
    FOREIGN KEY(set_id,archive_order) REFERENCES logiqx_archive_references(set_id,archive_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE logiqx_device_reference_attribute_positions (
    set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0),
    reference_order INTEGER NOT NULL CHECK(typeof(reference_order)='integer' AND reference_order>=0),
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind)='integer' AND field_kind=0),
    source_order INTEGER NOT NULL CHECK (typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(set_id,reference_order,field_kind), UNIQUE(set_id,reference_order,source_order),
    FOREIGN KEY(set_id,reference_order) REFERENCES logiqx_device_references(set_id,reference_order) ON DELETE RESTRICT
) WITHOUT ROWID;

-- The expected relation is also the single presence mapping used by closure
-- validation and the standalone semantic integrity view.
CREATE VIEW logiqx_expected_attribute_positions AS
SELECT snapshot_key,'document' AS owner_kind,snapshot_key AS owner_a,0 AS owner_b,field_kind
FROM logiqx_document_facts JOIN catalog_snapshots USING(snapshot_key)
JOIN parser_interpretations USING(interpretation_key)
JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1)
WHERE format='logiqx' AND CASE field_kind WHEN 0 THEN build IS NOT NULL WHEN 1 THEN debug_was_present=1 END
UNION ALL SELECT options.snapshot_key,'clrmamepro',options.snapshot_key,0,field_kind FROM logiqx_clrmamepro_options AS options
JOIN catalog_snapshots USING(snapshot_key) JOIN parser_interpretations USING(interpretation_key)
JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3)
WHERE format='logiqx' AND CASE field_kind WHEN 0 THEN header IS NOT NULL WHEN 1 THEN forcemerging_was_present=1
 WHEN 2 THEN forcenodump_was_present=1 WHEN 3 THEN forcepacking_was_present=1 END
UNION ALL SELECT options.snapshot_key,'romcenter',options.snapshot_key,0,field_kind FROM logiqx_romcenter_options AS options
JOIN catalog_snapshots USING(snapshot_key) JOIN parser_interpretations USING(interpretation_key)
JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4 UNION ALL SELECT 5 UNION ALL SELECT 6)
WHERE format='logiqx' AND CASE field_kind WHEN 0 THEN plugin IS NOT NULL WHEN 1 THEN rommode_was_present=1
 WHEN 2 THEN biosmode_was_present=1 WHEN 3 THEN samplemode_was_present=1
 WHEN 4 THEN lockrommode_was_present=1 WHEN 5 THEN lockbiosmode_was_present=1 WHEN 6 THEN locksamplemode_was_present=1 END
UNION ALL SELECT groups.snapshot_key,'game',game.set_id,0,field_kind FROM logiqx_games AS game
JOIN catalog_sets AS sets USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id)
JOIN catalog_snapshots USING(snapshot_key) JOIN parser_interpretations USING(interpretation_key)
JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4 UNION ALL SELECT 5 UNION ALL SELECT 6 UNION ALL SELECT 7)
WHERE sets.source_element_kind='logiqx_game' AND groups.kind='root' AND format='logiqx'
AND CASE field_kind WHEN 0 THEN sets.set_name IS NOT NULL WHEN 1 THEN game.source_file IS NOT NULL
 WHEN 2 THEN game.is_bios_was_present=1 WHEN 3 THEN EXISTS(SELECT 1 FROM logiqx_set_links WHERE set_id=game.set_id AND link_kind='cloneof')
 WHEN 4 THEN EXISTS(SELECT 1 FROM logiqx_set_links WHERE set_id=game.set_id AND link_kind='romof')
 WHEN 5 THEN EXISTS(SELECT 1 FROM logiqx_set_links WHERE set_id=game.set_id AND link_kind='sampleof')
 WHEN 6 THEN game.board IS NOT NULL WHEN 7 THEN game.rebuild_to IS NOT NULL END
UNION ALL SELECT groups.snapshot_key,'release',release.set_id,release.release_order,field_kind FROM logiqx_releases AS release
JOIN catalog_sets AS sets USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id)
JOIN logiqx_games AS game USING(set_id)
JOIN catalog_snapshots USING(snapshot_key) JOIN parser_interpretations USING(interpretation_key)
JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4)
WHERE sets.source_element_kind='logiqx_game' AND groups.kind='root' AND format='logiqx'
AND CASE field_kind WHEN 0 THEN release.name IS NOT NULL WHEN 1 THEN release.region IS NOT NULL
 WHEN 2 THEN release.language IS NOT NULL WHEN 3 THEN release.date IS NOT NULL WHEN 4 THEN release.default_was_present=1 END
UNION ALL SELECT groups.snapshot_key,'bios',bios.set_id,bios.bios_order,field_kind FROM logiqx_bios_sets AS bios
JOIN catalog_sets AS sets USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id)
JOIN logiqx_games AS game USING(set_id)
JOIN catalog_snapshots USING(snapshot_key) JOIN parser_interpretations USING(interpretation_key)
JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2)
WHERE sets.source_element_kind='logiqx_game' AND groups.kind='root' AND format='logiqx'
AND CASE field_kind WHEN 0 THEN bios.name IS NOT NULL WHEN 1 THEN bios.description IS NOT NULL WHEN 2 THEN bios.default_was_present=1 END
UNION ALL SELECT groups.snapshot_key,'rom',occurrence.occurrence_id,0,field_kind FROM logiqx_rom_claims AS rom
JOIN asset_occurrences AS occurrence USING(occurrence_id) JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
JOIN catalog_set_groups AS groups USING(set_group_id)
JOIN logiqx_games AS game USING(set_id)
JOIN catalog_snapshots USING(snapshot_key) JOIN parser_interpretations USING(interpretation_key)
JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4 UNION ALL SELECT 5 UNION ALL SELECT 6 UNION ALL SELECT 7 UNION ALL SELECT 8)
WHERE occurrence.claim_kind='logiqx_rom' AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND format='logiqx'
AND CASE field_kind WHEN 0 THEN rom.name IS NOT NULL WHEN 1 THEN rom.size_text IS NOT NULL
 WHEN 2 THEN rom.crc_text IS NOT NULL WHEN 3 THEN rom.sha1_text IS NOT NULL WHEN 4 THEN rom.md5_text IS NOT NULL
 WHEN 5 THEN EXISTS(SELECT 1 FROM logiqx_file_merges WHERE occurrence_id=rom.occurrence_id)
 WHEN 6 THEN rom.status_was_present=1 WHEN 7 THEN rom.date IS NOT NULL WHEN 8 THEN rom.serial IS NOT NULL END
UNION ALL SELECT groups.snapshot_key,'disk',occurrence.occurrence_id,0,field_kind FROM logiqx_disk_claims AS disk
JOIN asset_occurrences AS occurrence USING(occurrence_id) JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
JOIN catalog_set_groups AS groups USING(set_group_id)
JOIN logiqx_games AS game USING(set_id)
JOIN catalog_snapshots USING(snapshot_key) JOIN parser_interpretations USING(interpretation_key)
JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4)
WHERE occurrence.claim_kind='logiqx_disk' AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND format='logiqx'
AND CASE field_kind WHEN 0 THEN disk.name IS NOT NULL WHEN 1 THEN disk.sha1_text IS NOT NULL
 WHEN 2 THEN disk.md5_text IS NOT NULL WHEN 3 THEN EXISTS(SELECT 1 FROM logiqx_file_merges WHERE occurrence_id=disk.occurrence_id)
 WHEN 4 THEN disk.status_was_present=1 END
UNION ALL SELECT groups.snapshot_key,'sample',occurrence.occurrence_id,0,0 FROM logiqx_sample_claims AS sample
JOIN asset_occurrences AS occurrence USING(occurrence_id) JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
JOIN catalog_set_groups AS groups USING(set_group_id)
JOIN logiqx_games AS game USING(set_id)
JOIN catalog_snapshots USING(snapshot_key) JOIN parser_interpretations USING(interpretation_key)
WHERE occurrence.claim_kind='logiqx_sample' AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND format='logiqx'
UNION ALL SELECT groups.snapshot_key,'archive',archive.set_id,archive.archive_order,0 FROM logiqx_archive_references AS archive
JOIN catalog_sets AS sets USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id)
JOIN logiqx_games AS game USING(set_id)
JOIN catalog_snapshots USING(snapshot_key) JOIN parser_interpretations USING(interpretation_key)
WHERE sets.source_element_kind='logiqx_game' AND groups.kind='root' AND format='logiqx'
UNION ALL SELECT groups.snapshot_key,'device_reference',device.set_id,device.reference_order,0 FROM logiqx_device_references AS device
JOIN catalog_sets AS sets USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id)
JOIN logiqx_games AS game USING(set_id)
JOIN catalog_snapshots USING(snapshot_key) JOIN parser_interpretations USING(interpretation_key)
WHERE sets.source_element_kind='logiqx_game' AND groups.kind='root' AND format='logiqx';

CREATE VIEW logiqx_actual_attribute_positions AS
SELECT snapshot_key,'document' AS owner_kind,snapshot_key AS owner_a,0 AS owner_b,field_kind FROM logiqx_document_attribute_positions
UNION ALL SELECT snapshot_key,'clrmamepro',snapshot_key,0,field_kind FROM logiqx_clrmamepro_attribute_positions
UNION ALL SELECT snapshot_key,'romcenter',snapshot_key,0,field_kind FROM logiqx_romcenter_attribute_positions
UNION ALL SELECT groups.snapshot_key,'game',position.set_id,0,position.field_kind FROM logiqx_game_attribute_positions AS position
JOIN catalog_sets AS sets USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id)
UNION ALL SELECT groups.snapshot_key,'release',position.set_id,position.release_order,position.field_kind FROM logiqx_release_attribute_positions AS position
JOIN catalog_sets AS sets USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id)
UNION ALL SELECT groups.snapshot_key,'bios',position.set_id,position.bios_order,position.field_kind FROM logiqx_bios_attribute_positions AS position
JOIN catalog_sets AS sets USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id)
UNION ALL SELECT groups.snapshot_key,'rom',position.occurrence_id,0,position.field_kind FROM logiqx_rom_attribute_positions AS position
JOIN asset_occurrences AS occurrence USING(occurrence_id) JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
JOIN catalog_set_groups AS groups USING(set_group_id)
UNION ALL SELECT groups.snapshot_key,'disk',position.occurrence_id,0,position.field_kind FROM logiqx_disk_attribute_positions AS position
JOIN asset_occurrences AS occurrence USING(occurrence_id) JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
JOIN catalog_set_groups AS groups USING(set_group_id)
UNION ALL SELECT groups.snapshot_key,'sample',position.occurrence_id,0,position.field_kind FROM logiqx_sample_attribute_positions AS position
JOIN asset_occurrences AS occurrence USING(occurrence_id) JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
JOIN catalog_set_groups AS groups USING(set_group_id)
UNION ALL SELECT groups.snapshot_key,'archive',position.set_id,position.archive_order,position.field_kind FROM logiqx_archive_attribute_positions AS position
JOIN catalog_sets AS sets USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id)
UNION ALL SELECT groups.snapshot_key,'device_reference',position.set_id,position.reference_order,position.field_kind FROM logiqx_device_reference_attribute_positions AS position
JOIN catalog_sets AS sets USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id);

-- Global audit intentionally includes drafts and rows whose composite payload
-- was manually removed with foreign keys disabled.
CREATE VIEW logiqx_attribute_violations AS
SELECT expected.snapshot_key,expected.owner_kind,expected.owner_a,expected.owner_b,expected.field_kind,'missing_position' AS reason
FROM logiqx_expected_attribute_positions AS expected
WHERE NOT EXISTS (SELECT 1 FROM logiqx_actual_attribute_positions AS actual
 WHERE actual.snapshot_key=expected.snapshot_key AND actual.owner_kind=expected.owner_kind
 AND actual.owner_a=expected.owner_a AND actual.owner_b=expected.owner_b AND actual.field_kind=expected.field_kind)
UNION ALL
SELECT groups.snapshot_key,'device_reference',reference.set_id,reference.reference_order,0,'invalid_element_position'
FROM logiqx_device_references AS reference
LEFT JOIN catalog_sets AS sets USING(set_id)
LEFT JOIN catalog_set_groups AS groups USING(set_group_id)
WHERE typeof(reference.reference_order)<>'integer' OR reference.reference_order<0
   OR typeof(reference.source_order)<>'integer' OR reference.source_order<0
   OR typeof(reference.source_line)<>'integer' OR reference.source_line<=0
   OR typeof(reference.source_column)<>'integer' OR reference.source_column<=0
UNION ALL
SELECT actual.snapshot_key,actual.owner_kind,actual.owner_a,actual.owner_b,actual.field_kind,'extraneous_or_misplaced_position'
FROM logiqx_actual_attribute_positions AS actual
WHERE NOT EXISTS (SELECT 1 FROM logiqx_expected_attribute_positions AS expected
 WHERE expected.snapshot_key=actual.snapshot_key AND expected.owner_kind=actual.owner_kind
 AND expected.owner_a=actual.owner_a AND expected.owner_b=actual.owner_b AND expected.field_kind=actual.field_kind)
UNION ALL
SELECT position.snapshot_key,'document',position.snapshot_key,0,position.field_kind,'orphan_or_wrong_ancestry'
FROM logiqx_document_attribute_positions AS position
WHERE NOT EXISTS(SELECT 1 FROM logiqx_document_facts AS owner JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN parser_interpretations USING(interpretation_key) WHERE owner.snapshot_key=position.snapshot_key AND format='logiqx')
UNION ALL
SELECT position.snapshot_key,'clrmamepro',position.snapshot_key,0,position.field_kind,'orphan_or_wrong_ancestry'
FROM logiqx_clrmamepro_attribute_positions AS position
WHERE NOT EXISTS(SELECT 1 FROM logiqx_clrmamepro_options AS owner JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN parser_interpretations USING(interpretation_key) WHERE owner.snapshot_key=position.snapshot_key AND format='logiqx')
UNION ALL
SELECT position.snapshot_key,'romcenter',position.snapshot_key,0,position.field_kind,'orphan_or_wrong_ancestry'
FROM logiqx_romcenter_attribute_positions AS position
WHERE NOT EXISTS(SELECT 1 FROM logiqx_romcenter_options AS owner JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN parser_interpretations USING(interpretation_key) WHERE owner.snapshot_key=position.snapshot_key AND format='logiqx')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id) WHERE sets.set_id=position.set_id),
 'game',position.set_id,0,position.field_kind,'orphan_or_wrong_ancestry'
FROM logiqx_game_attribute_positions AS position
WHERE NOT EXISTS(SELECT 1 FROM logiqx_games AS owner JOIN catalog_sets AS sets USING(set_id)
 JOIN catalog_set_groups AS groups USING(set_group_id) JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN parser_interpretations USING(interpretation_key) WHERE owner.set_id=position.set_id
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND format='logiqx')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id) WHERE sets.set_id=position.set_id),
 'release',position.set_id,position.release_order,position.field_kind,'orphan_or_wrong_ancestry'
FROM logiqx_release_attribute_positions AS position
WHERE NOT EXISTS(SELECT 1 FROM logiqx_releases AS owner JOIN catalog_sets AS sets USING(set_id)
 JOIN catalog_set_groups AS groups USING(set_group_id) JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN parser_interpretations USING(interpretation_key) WHERE owner.set_id=position.set_id AND owner.release_order=position.release_order
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND format='logiqx')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id) WHERE sets.set_id=position.set_id),
 'bios',position.set_id,position.bios_order,position.field_kind,'orphan_or_wrong_ancestry'
FROM logiqx_bios_attribute_positions AS position
WHERE NOT EXISTS(SELECT 1 FROM logiqx_bios_sets AS owner JOIN catalog_sets AS sets USING(set_id)
 JOIN catalog_set_groups AS groups USING(set_group_id) JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN parser_interpretations USING(interpretation_key) WHERE owner.set_id=position.set_id AND owner.bios_order=position.bios_order
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND format='logiqx')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM asset_occurrences AS occurrence JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN catalog_set_groups AS groups USING(set_group_id) WHERE occurrence.occurrence_id=position.occurrence_id),
 'rom',position.occurrence_id,0,position.field_kind,'orphan_or_wrong_ancestry'
FROM logiqx_rom_attribute_positions AS position
WHERE NOT EXISTS(SELECT 1 FROM logiqx_rom_claims AS owner JOIN asset_occurrences AS occurrence USING(occurrence_id)
 JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN catalog_snapshots AS snapshot USING(snapshot_key) JOIN parser_interpretations USING(interpretation_key)
 WHERE owner.occurrence_id=position.occurrence_id AND occurrence.claim_kind='logiqx_rom'
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND format='logiqx')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM asset_occurrences AS occurrence JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN catalog_set_groups AS groups USING(set_group_id) WHERE occurrence.occurrence_id=position.occurrence_id),
 'disk',position.occurrence_id,0,position.field_kind,'orphan_or_wrong_ancestry'
FROM logiqx_disk_attribute_positions AS position
WHERE NOT EXISTS(SELECT 1 FROM logiqx_disk_claims AS owner JOIN asset_occurrences AS occurrence USING(occurrence_id)
 JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN catalog_snapshots AS snapshot USING(snapshot_key) JOIN parser_interpretations USING(interpretation_key)
 WHERE owner.occurrence_id=position.occurrence_id AND occurrence.claim_kind='logiqx_disk'
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND format='logiqx')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM asset_occurrences AS occurrence JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN catalog_set_groups AS groups USING(set_group_id) WHERE occurrence.occurrence_id=position.occurrence_id),
 'sample',position.occurrence_id,0,position.field_kind,'orphan_or_wrong_ancestry'
FROM logiqx_sample_attribute_positions AS position
WHERE NOT EXISTS(SELECT 1 FROM logiqx_sample_claims AS owner JOIN asset_occurrences AS occurrence USING(occurrence_id)
 JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN catalog_snapshots AS snapshot USING(snapshot_key) JOIN parser_interpretations USING(interpretation_key)
 WHERE owner.occurrence_id=position.occurrence_id AND occurrence.claim_kind='logiqx_sample'
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND format='logiqx')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id) WHERE sets.set_id=position.set_id),
 'archive',position.set_id,position.archive_order,position.field_kind,'orphan_or_wrong_ancestry'
FROM logiqx_archive_attribute_positions AS position
WHERE NOT EXISTS(SELECT 1 FROM logiqx_archive_references AS owner JOIN catalog_sets AS sets USING(set_id)
 JOIN catalog_set_groups AS groups USING(set_group_id) JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN parser_interpretations USING(interpretation_key) WHERE owner.set_id=position.set_id AND owner.archive_order=position.archive_order
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND format='logiqx')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id) WHERE sets.set_id=position.set_id),
 'device_reference',position.set_id,position.reference_order,position.field_kind,'orphan_or_wrong_ancestry'
FROM logiqx_device_reference_attribute_positions AS position
WHERE NOT EXISTS(SELECT 1 FROM logiqx_device_references AS owner JOIN catalog_sets AS sets USING(set_id)
 JOIN catalog_set_groups AS groups USING(set_group_id) JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN parser_interpretations USING(interpretation_key) WHERE owner.set_id=position.set_id AND owner.reference_order=position.reference_order
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND format='logiqx')
UNION ALL
SELECT owner.snapshot_key,'document',owner.snapshot_key,0,0,'orphan_or_wrong_ancestry'
FROM logiqx_document_facts AS owner
WHERE NOT EXISTS(SELECT 1 FROM catalog_snapshots AS snapshot
 JOIN catalogs USING(catalog_key) JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE snapshot.snapshot_key=owner.snapshot_key AND parser.format='logiqx')
UNION ALL
SELECT owner.snapshot_key,'clrmamepro',owner.snapshot_key,0,0,'orphan_or_wrong_ancestry'
FROM logiqx_clrmamepro_options AS owner
WHERE NOT EXISTS(SELECT 1 FROM logiqx_document_facts AS facts
 JOIN catalog_snapshots AS snapshot USING(snapshot_key) JOIN catalogs USING(catalog_key)
 JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE facts.snapshot_key=owner.snapshot_key AND parser.format='logiqx')
UNION ALL
SELECT owner.snapshot_key,'romcenter',owner.snapshot_key,0,0,'orphan_or_wrong_ancestry'
FROM logiqx_romcenter_options AS owner
WHERE NOT EXISTS(SELECT 1 FROM logiqx_document_facts AS facts
 JOIN catalog_snapshots AS snapshot USING(snapshot_key) JOIN catalogs USING(catalog_key)
 JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE facts.snapshot_key=owner.snapshot_key AND parser.format='logiqx')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id)
 WHERE sets.set_id=owner.set_id),'game',owner.set_id,0,0,'orphan_or_wrong_ancestry'
FROM logiqx_games AS owner
WHERE NOT EXISTS(SELECT 1 FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN catalog_snapshots AS snapshot USING(snapshot_key) JOIN catalogs USING(catalog_key)
 JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE sets.set_id=owner.set_id AND sets.source_element_kind='logiqx_game'
 AND groups.kind='root' AND parser.format='logiqx')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id)
 WHERE sets.set_id=owner.set_id),'release',owner.set_id,owner.release_order,0,'orphan_or_wrong_ancestry'
FROM logiqx_releases AS owner
WHERE NOT EXISTS(SELECT 1 FROM logiqx_games AS game JOIN catalog_sets AS sets USING(set_id)
 JOIN catalog_set_groups AS groups USING(set_group_id) JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN catalogs USING(catalog_key) JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE game.set_id=owner.set_id AND sets.source_element_kind='logiqx_game'
 AND groups.kind='root' AND parser.format='logiqx')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id)
 WHERE sets.set_id=owner.set_id),'bios',owner.set_id,owner.bios_order,0,'orphan_or_wrong_ancestry'
FROM logiqx_bios_sets AS owner
WHERE NOT EXISTS(SELECT 1 FROM logiqx_games AS game JOIN catalog_sets AS sets USING(set_id)
 JOIN catalog_set_groups AS groups USING(set_group_id) JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN catalogs USING(catalog_key) JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE game.set_id=owner.set_id AND sets.source_element_kind='logiqx_game'
 AND groups.kind='root' AND parser.format='logiqx')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id)
 WHERE sets.set_id=owner.set_id),'archive',owner.set_id,owner.archive_order,0,'orphan_or_wrong_ancestry'
FROM logiqx_archive_references AS owner
WHERE NOT EXISTS(SELECT 1 FROM logiqx_games AS game JOIN catalog_sets AS sets USING(set_id)
 JOIN catalog_set_groups AS groups USING(set_group_id) JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN catalogs USING(catalog_key) JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE game.set_id=owner.set_id AND sets.source_element_kind='logiqx_game'
 AND groups.kind='root' AND parser.format='logiqx')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id)
 WHERE sets.set_id=owner.set_id),'device_reference',owner.set_id,owner.reference_order,0,'orphan_or_wrong_ancestry'
FROM logiqx_device_references AS owner
WHERE NOT EXISTS(SELECT 1 FROM logiqx_games AS game JOIN catalog_sets AS sets USING(set_id)
 JOIN catalog_set_groups AS groups USING(set_group_id) JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN catalogs USING(catalog_key) JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE game.set_id=owner.set_id AND sets.source_element_kind='logiqx_game'
 AND groups.kind='root' AND parser.format='logiqx')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM asset_occurrences AS occurrence
 JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN catalog_set_groups AS groups USING(set_group_id)
 WHERE occurrence.occurrence_id=owner.occurrence_id),'rom',owner.occurrence_id,0,0,'orphan_or_wrong_ancestry'
FROM logiqx_rom_claims AS owner
WHERE NOT EXISTS(SELECT 1 FROM asset_occurrences AS occurrence JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
 JOIN logiqx_games AS game USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN catalog_snapshots AS snapshot USING(snapshot_key) JOIN catalogs USING(catalog_key)
 JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE occurrence.occurrence_id=owner.occurrence_id AND occurrence.claim_kind='logiqx_rom'
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND parser.format='logiqx')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM asset_occurrences AS occurrence
 JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN catalog_set_groups AS groups USING(set_group_id)
 WHERE occurrence.occurrence_id=owner.occurrence_id),'disk',owner.occurrence_id,0,0,'orphan_or_wrong_ancestry'
FROM logiqx_disk_claims AS owner
WHERE NOT EXISTS(SELECT 1 FROM asset_occurrences AS occurrence JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
 JOIN logiqx_games AS game USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN catalog_snapshots AS snapshot USING(snapshot_key) JOIN catalogs USING(catalog_key)
 JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE occurrence.occurrence_id=owner.occurrence_id AND occurrence.claim_kind='logiqx_disk'
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND parser.format='logiqx')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM asset_occurrences AS occurrence
 JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN catalog_set_groups AS groups USING(set_group_id)
 WHERE occurrence.occurrence_id=owner.occurrence_id),'sample',owner.occurrence_id,0,0,'orphan_or_wrong_ancestry'
FROM logiqx_sample_claims AS owner
WHERE NOT EXISTS(SELECT 1 FROM asset_occurrences AS occurrence JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
 JOIN logiqx_games AS game USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN catalog_snapshots AS snapshot USING(snapshot_key) JOIN catalogs USING(catalog_key)
 JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE occurrence.occurrence_id=owner.occurrence_id AND occurrence.claim_kind='logiqx_sample'
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND parser.format='logiqx');

-- Position insertion checks actual value owners and their full format ancestry;
-- these checks do not depend on either SQLite pragma.
CREATE TRIGGER logiqx_document_attribute_positions_insert_guard
BEFORE INSERT ON logiqx_document_attribute_positions
WHEN EXISTS(SELECT 1 FROM logiqx_document_attribute_positions AS old WHERE old.snapshot_key=NEW.snapshot_key AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM logiqx_document_facts AS owner JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN parser_interpretations AS parser USING(interpretation_key) WHERE owner.snapshot_key=NEW.snapshot_key AND parser.format='logiqx'
 AND EXISTS(SELECT 1 FROM catalogs WHERE catalog_key=snapshot.catalog_key)
 AND EXISTS(SELECT 1 FROM documents WHERE document_key=snapshot.document_key)
 AND CASE NEW.field_kind WHEN 0 THEN owner.build IS NOT NULL WHEN 1 THEN owner.debug_was_present=1 ELSE 0 END)
 OR EXISTS(SELECT 1 FROM snapshot_publications WHERE snapshot_key=NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT,'Logiqx document attribute position requires a present field on an unpublished owner'); END;
CREATE TRIGGER logiqx_clrmamepro_attribute_positions_insert_guard
BEFORE INSERT ON logiqx_clrmamepro_attribute_positions
WHEN EXISTS(SELECT 1 FROM logiqx_clrmamepro_attribute_positions AS old WHERE old.snapshot_key=NEW.snapshot_key AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM logiqx_clrmamepro_options AS owner JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN parser_interpretations AS parser USING(interpretation_key) WHERE owner.snapshot_key=NEW.snapshot_key AND parser.format='logiqx'
 AND EXISTS(SELECT 1 FROM catalogs WHERE catalog_key=snapshot.catalog_key)
 AND EXISTS(SELECT 1 FROM documents WHERE document_key=snapshot.document_key)
 AND CASE NEW.field_kind WHEN 0 THEN owner.header IS NOT NULL WHEN 1 THEN owner.forcemerging_was_present=1
 WHEN 2 THEN owner.forcenodump_was_present=1 WHEN 3 THEN owner.forcepacking_was_present=1 ELSE 0 END)
 OR EXISTS(SELECT 1 FROM snapshot_publications WHERE snapshot_key=NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT,'Logiqx clrmamepro attribute position requires a present field on an unpublished owner'); END;
CREATE TRIGGER logiqx_romcenter_attribute_positions_insert_guard
BEFORE INSERT ON logiqx_romcenter_attribute_positions
WHEN EXISTS(SELECT 1 FROM logiqx_romcenter_attribute_positions AS old WHERE old.snapshot_key=NEW.snapshot_key AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM logiqx_romcenter_options AS owner JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN parser_interpretations AS parser USING(interpretation_key) WHERE owner.snapshot_key=NEW.snapshot_key AND parser.format='logiqx'
 AND EXISTS(SELECT 1 FROM catalogs WHERE catalog_key=snapshot.catalog_key)
 AND EXISTS(SELECT 1 FROM documents WHERE document_key=snapshot.document_key)
 AND CASE NEW.field_kind WHEN 0 THEN owner.plugin IS NOT NULL WHEN 1 THEN owner.rommode_was_present=1
 WHEN 2 THEN owner.biosmode_was_present=1 WHEN 3 THEN owner.samplemode_was_present=1
 WHEN 4 THEN owner.lockrommode_was_present=1 WHEN 5 THEN owner.lockbiosmode_was_present=1 WHEN 6 THEN owner.locksamplemode_was_present=1 ELSE 0 END)
 OR EXISTS(SELECT 1 FROM snapshot_publications WHERE snapshot_key=NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT,'Logiqx romcenter attribute position requires a present field on an unpublished owner'); END;

CREATE TRIGGER logiqx_game_attribute_positions_insert_guard
BEFORE INSERT ON logiqx_game_attribute_positions
WHEN EXISTS(SELECT 1 FROM logiqx_game_attribute_positions AS old WHERE old.set_id=NEW.set_id AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM logiqx_games AS owner JOIN catalog_sets AS sets USING(set_id)
 JOIN catalog_set_groups AS groups USING(set_group_id) JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN parser_interpretations AS parser USING(interpretation_key) WHERE owner.set_id=NEW.set_id
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND parser.format='logiqx'
 AND EXISTS(SELECT 1 FROM catalogs WHERE catalog_key=snapshot.catalog_key)
 AND EXISTS(SELECT 1 FROM documents WHERE document_key=snapshot.document_key)
 AND CASE NEW.field_kind WHEN 0 THEN sets.set_name IS NOT NULL WHEN 1 THEN owner.source_file IS NOT NULL
 WHEN 2 THEN owner.is_bios_was_present=1 WHEN 3 THEN EXISTS(SELECT 1 FROM logiqx_set_links WHERE set_id=owner.set_id AND link_kind='cloneof')
 WHEN 4 THEN EXISTS(SELECT 1 FROM logiqx_set_links WHERE set_id=owner.set_id AND link_kind='romof')
 WHEN 5 THEN EXISTS(SELECT 1 FROM logiqx_set_links WHERE set_id=owner.set_id AND link_kind='sampleof')
 WHEN 6 THEN owner.board IS NOT NULL WHEN 7 THEN owner.rebuild_to IS NOT NULL ELSE 0 END)
 OR EXISTS(SELECT 1 FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN snapshot_publications USING(snapshot_key) WHERE sets.set_id=NEW.set_id)
BEGIN SELECT RAISE(ABORT,'Logiqx game attribute position requires its present field on an unpublished game'); END;

CREATE TRIGGER logiqx_release_attribute_positions_insert_guard
BEFORE INSERT ON logiqx_release_attribute_positions
WHEN EXISTS(SELECT 1 FROM logiqx_release_attribute_positions AS old WHERE old.set_id=NEW.set_id AND old.release_order=NEW.release_order AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM logiqx_releases AS owner JOIN catalog_sets AS sets USING(set_id)
 JOIN logiqx_games AS game USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id) JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN parser_interpretations AS parser USING(interpretation_key) WHERE owner.set_id=NEW.set_id AND owner.release_order=NEW.release_order
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND parser.format='logiqx'
 AND EXISTS(SELECT 1 FROM catalogs WHERE catalog_key=snapshot.catalog_key)
 AND EXISTS(SELECT 1 FROM documents WHERE document_key=snapshot.document_key)
 AND CASE NEW.field_kind WHEN 0 THEN owner.name IS NOT NULL WHEN 1 THEN owner.region IS NOT NULL
 WHEN 2 THEN owner.language IS NOT NULL WHEN 3 THEN owner.date IS NOT NULL WHEN 4 THEN owner.default_was_present=1 ELSE 0 END)
 OR EXISTS(SELECT 1 FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN snapshot_publications USING(snapshot_key) WHERE sets.set_id=NEW.set_id)
BEGIN SELECT RAISE(ABORT,'Logiqx release attribute position requires its present field on an unpublished release'); END;
CREATE TRIGGER logiqx_bios_attribute_positions_insert_guard
BEFORE INSERT ON logiqx_bios_attribute_positions
WHEN EXISTS(SELECT 1 FROM logiqx_bios_attribute_positions AS old WHERE old.set_id=NEW.set_id AND old.bios_order=NEW.bios_order AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM logiqx_bios_sets AS owner JOIN catalog_sets AS sets USING(set_id)
 JOIN logiqx_games AS game USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id) JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN parser_interpretations AS parser USING(interpretation_key) WHERE owner.set_id=NEW.set_id AND owner.bios_order=NEW.bios_order
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND parser.format='logiqx'
 AND EXISTS(SELECT 1 FROM catalogs WHERE catalog_key=snapshot.catalog_key)
 AND EXISTS(SELECT 1 FROM documents WHERE document_key=snapshot.document_key)
 AND CASE NEW.field_kind WHEN 0 THEN owner.name IS NOT NULL WHEN 1 THEN owner.description IS NOT NULL WHEN 2 THEN owner.default_was_present=1 ELSE 0 END)
 OR EXISTS(SELECT 1 FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN snapshot_publications USING(snapshot_key) WHERE sets.set_id=NEW.set_id)
BEGIN SELECT RAISE(ABORT,'Logiqx BIOS attribute position requires its present field on an unpublished BIOS set'); END;

CREATE TRIGGER logiqx_rom_attribute_positions_insert_guard
BEFORE INSERT ON logiqx_rom_attribute_positions
WHEN EXISTS(SELECT 1 FROM logiqx_rom_attribute_positions AS old WHERE old.occurrence_id=NEW.occurrence_id AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM logiqx_rom_claims AS owner JOIN asset_occurrences AS occurrence USING(occurrence_id)
 JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN logiqx_games AS game ON game.set_id=sets.set_id JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN catalog_snapshots AS snapshot USING(snapshot_key) JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE owner.occurrence_id=NEW.occurrence_id AND occurrence.claim_kind='logiqx_rom' AND sets.source_element_kind='logiqx_game'
 AND groups.kind='root' AND parser.format='logiqx'
 AND EXISTS(SELECT 1 FROM catalogs WHERE catalog_key=snapshot.catalog_key)
 AND EXISTS(SELECT 1 FROM documents WHERE document_key=snapshot.document_key)
 AND CASE NEW.field_kind
 WHEN 0 THEN owner.name IS NOT NULL WHEN 1 THEN owner.size_text IS NOT NULL WHEN 2 THEN owner.crc_text IS NOT NULL
 WHEN 3 THEN owner.sha1_text IS NOT NULL WHEN 4 THEN owner.md5_text IS NOT NULL
 WHEN 5 THEN EXISTS(SELECT 1 FROM logiqx_file_merges WHERE occurrence_id=owner.occurrence_id)
 WHEN 6 THEN owner.status_was_present=1 WHEN 7 THEN owner.date IS NOT NULL WHEN 8 THEN owner.serial IS NOT NULL ELSE 0 END)
 OR EXISTS(SELECT 1 FROM asset_occurrences AS occurrence JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
 JOIN catalog_set_groups AS groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key)
 WHERE occurrence.occurrence_id=NEW.occurrence_id)
BEGIN SELECT RAISE(ABORT,'Logiqx ROM attribute position requires its present field on an unpublished ROM'); END;
CREATE TRIGGER logiqx_disk_attribute_positions_insert_guard
BEFORE INSERT ON logiqx_disk_attribute_positions
WHEN EXISTS(SELECT 1 FROM logiqx_disk_attribute_positions AS old WHERE old.occurrence_id=NEW.occurrence_id AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM logiqx_disk_claims AS owner JOIN asset_occurrences AS occurrence USING(occurrence_id)
 JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN logiqx_games AS game ON game.set_id=sets.set_id JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN catalog_snapshots AS snapshot USING(snapshot_key) JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE owner.occurrence_id=NEW.occurrence_id AND occurrence.claim_kind='logiqx_disk' AND sets.source_element_kind='logiqx_game'
 AND groups.kind='root' AND parser.format='logiqx'
 AND EXISTS(SELECT 1 FROM catalogs WHERE catalog_key=snapshot.catalog_key)
 AND EXISTS(SELECT 1 FROM documents WHERE document_key=snapshot.document_key)
 AND CASE NEW.field_kind
 WHEN 0 THEN owner.name IS NOT NULL WHEN 1 THEN owner.sha1_text IS NOT NULL WHEN 2 THEN owner.md5_text IS NOT NULL
 WHEN 3 THEN EXISTS(SELECT 1 FROM logiqx_file_merges WHERE occurrence_id=owner.occurrence_id)
 WHEN 4 THEN owner.status_was_present=1 ELSE 0 END)
 OR EXISTS(SELECT 1 FROM asset_occurrences AS occurrence JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
 JOIN catalog_set_groups AS groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key)
 WHERE occurrence.occurrence_id=NEW.occurrence_id)
BEGIN SELECT RAISE(ABORT,'Logiqx disk attribute position requires its present field on an unpublished disk'); END;
CREATE TRIGGER logiqx_sample_attribute_positions_insert_guard
BEFORE INSERT ON logiqx_sample_attribute_positions
WHEN EXISTS(SELECT 1 FROM logiqx_sample_attribute_positions AS old WHERE old.occurrence_id=NEW.occurrence_id AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM logiqx_sample_claims AS owner JOIN asset_occurrences AS occurrence USING(occurrence_id)
 JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN logiqx_games AS game ON game.set_id=sets.set_id JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN catalog_snapshots AS snapshot USING(snapshot_key) JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE owner.occurrence_id=NEW.occurrence_id AND occurrence.claim_kind='logiqx_sample' AND sets.source_element_kind='logiqx_game'
 AND groups.kind='root' AND parser.format='logiqx'
 AND EXISTS(SELECT 1 FROM catalogs WHERE catalog_key=snapshot.catalog_key)
 AND EXISTS(SELECT 1 FROM documents WHERE document_key=snapshot.document_key)
 AND NEW.field_kind=0)
 OR EXISTS(SELECT 1 FROM asset_occurrences AS occurrence JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
 JOIN catalog_set_groups AS groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key)
 WHERE occurrence.occurrence_id=NEW.occurrence_id)
BEGIN SELECT RAISE(ABORT,'Logiqx sample attribute position requires its present field on an unpublished sample'); END;

CREATE TRIGGER logiqx_archive_attribute_positions_insert_guard
BEFORE INSERT ON logiqx_archive_attribute_positions
WHEN EXISTS(SELECT 1 FROM logiqx_archive_attribute_positions AS old WHERE old.set_id=NEW.set_id AND old.archive_order=NEW.archive_order AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM logiqx_archive_references AS owner JOIN catalog_sets AS sets USING(set_id)
 JOIN logiqx_games AS game USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id) JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN parser_interpretations AS parser USING(interpretation_key) WHERE owner.set_id=NEW.set_id AND owner.archive_order=NEW.archive_order
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND parser.format='logiqx'
 AND EXISTS(SELECT 1 FROM catalogs WHERE catalog_key=snapshot.catalog_key)
 AND EXISTS(SELECT 1 FROM documents WHERE document_key=snapshot.document_key)
 AND NEW.field_kind=0 AND owner.archive_name IS NOT NULL)
 OR EXISTS(SELECT 1 FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN snapshot_publications USING(snapshot_key) WHERE sets.set_id=NEW.set_id)
BEGIN SELECT RAISE(ABORT,'Logiqx archive attribute position requires its present field on an unpublished archive reference'); END;
CREATE TRIGGER logiqx_device_reference_attribute_positions_insert_guard
BEFORE INSERT ON logiqx_device_reference_attribute_positions
WHEN EXISTS(SELECT 1 FROM logiqx_device_reference_attribute_positions AS old WHERE old.set_id=NEW.set_id AND old.reference_order=NEW.reference_order AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM logiqx_device_references AS owner JOIN catalog_sets AS sets USING(set_id)
 JOIN logiqx_games AS game USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id) JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN parser_interpretations AS parser USING(interpretation_key) WHERE owner.set_id=NEW.set_id AND owner.reference_order=NEW.reference_order
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND parser.format='logiqx'
 AND EXISTS(SELECT 1 FROM catalogs WHERE catalog_key=snapshot.catalog_key)
 AND EXISTS(SELECT 1 FROM documents WHERE document_key=snapshot.document_key)
 AND NEW.field_kind=0 AND owner.target_name IS NOT NULL)
 OR EXISTS(SELECT 1 FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN snapshot_publications USING(snapshot_key) WHERE sets.set_id=NEW.set_id)
BEGIN SELECT RAISE(ABORT,'Logiqx device reference attribute position requires its present field on an unpublished device reference'); END;

CREATE TRIGGER logiqx_attribute_positions_immutable_update BEFORE UPDATE ON logiqx_document_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_attribute_positions_immutable_delete BEFORE DELETE ON logiqx_document_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_clrmamepro_positions_immutable_update BEFORE UPDATE ON logiqx_clrmamepro_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_clrmamepro_positions_immutable_delete BEFORE DELETE ON logiqx_clrmamepro_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_romcenter_positions_immutable_update BEFORE UPDATE ON logiqx_romcenter_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_romcenter_positions_immutable_delete BEFORE DELETE ON logiqx_romcenter_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_game_positions_immutable_update BEFORE UPDATE ON logiqx_game_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_game_positions_immutable_delete BEFORE DELETE ON logiqx_game_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_release_positions_immutable_update BEFORE UPDATE ON logiqx_release_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_release_positions_immutable_delete BEFORE DELETE ON logiqx_release_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_bios_positions_immutable_update BEFORE UPDATE ON logiqx_bios_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_bios_positions_immutable_delete BEFORE DELETE ON logiqx_bios_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_rom_positions_immutable_update BEFORE UPDATE ON logiqx_rom_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_rom_positions_immutable_delete BEFORE DELETE ON logiqx_rom_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_disk_positions_immutable_update BEFORE UPDATE ON logiqx_disk_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_disk_positions_immutable_delete BEFORE DELETE ON logiqx_disk_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_sample_positions_immutable_update BEFORE UPDATE ON logiqx_sample_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_sample_positions_immutable_delete BEFORE DELETE ON logiqx_sample_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_archive_positions_immutable_update BEFORE UPDATE ON logiqx_archive_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_archive_positions_immutable_delete BEFORE DELETE ON logiqx_archive_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_device_positions_immutable_update BEFORE UPDATE ON logiqx_device_reference_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;
CREATE TRIGGER logiqx_device_positions_immutable_delete BEFORE DELETE ON logiqx_device_reference_attribute_positions BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;

CREATE TRIGGER logiqx_attribute_positions_publication_guard
BEFORE INSERT ON snapshot_publications
WHEN EXISTS(SELECT 1 FROM logiqx_expected_attribute_positions AS expected WHERE expected.snapshot_key=NEW.snapshot_key
 AND NOT EXISTS(SELECT 1 FROM logiqx_actual_attribute_positions AS actual WHERE actual.snapshot_key=expected.snapshot_key
 AND actual.owner_kind=expected.owner_kind AND actual.owner_a=expected.owner_a AND actual.owner_b=expected.owner_b AND actual.field_kind=expected.field_kind))
 OR EXISTS(SELECT 1 FROM logiqx_actual_attribute_positions AS actual WHERE actual.snapshot_key=NEW.snapshot_key
 AND NOT EXISTS(SELECT 1 FROM logiqx_expected_attribute_positions AS expected WHERE expected.snapshot_key=actual.snapshot_key
 AND expected.owner_kind=actual.owner_kind AND expected.owner_a=actual.owner_a AND expected.owner_b=actual.owner_b AND expected.field_kind=actual.field_kind))
 OR EXISTS(SELECT 1 FROM logiqx_document_facts AS owner
 WHERE owner.snapshot_key=NEW.snapshot_key AND NOT EXISTS(
  SELECT 1 FROM catalog_snapshots AS snapshot JOIN catalogs USING(catalog_key)
  JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
  JOIN parser_interpretations AS parser USING(interpretation_key)
  WHERE snapshot.snapshot_key=owner.snapshot_key AND parser.format='logiqx'))
 OR EXISTS(SELECT 1 FROM logiqx_clrmamepro_options AS owner
 WHERE owner.snapshot_key=NEW.snapshot_key AND NOT EXISTS(
  SELECT 1 FROM logiqx_document_facts AS facts JOIN catalog_snapshots AS snapshot USING(snapshot_key)
  JOIN catalogs USING(catalog_key) JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
  JOIN parser_interpretations AS parser USING(interpretation_key)
  WHERE facts.snapshot_key=owner.snapshot_key AND parser.format='logiqx'))
 OR EXISTS(SELECT 1 FROM logiqx_romcenter_options AS owner
 WHERE owner.snapshot_key=NEW.snapshot_key AND NOT EXISTS(
  SELECT 1 FROM logiqx_document_facts AS facts JOIN catalog_snapshots AS snapshot USING(snapshot_key)
  JOIN catalogs USING(catalog_key) JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
  JOIN parser_interpretations AS parser USING(interpretation_key)
  WHERE facts.snapshot_key=owner.snapshot_key AND parser.format='logiqx'))
 OR EXISTS(SELECT 1 FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING(set_group_id)
  JOIN logiqx_games AS owner USING(set_id)
  WHERE groups.snapshot_key=NEW.snapshot_key AND NOT EXISTS(
   SELECT 1 FROM catalog_snapshots AS snapshot JOIN catalogs USING(catalog_key)
   JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
   JOIN parser_interpretations AS parser USING(interpretation_key)
   WHERE snapshot.snapshot_key=groups.snapshot_key AND parser.format='logiqx'
   AND groups.kind='root' AND sets.source_element_kind='logiqx_game'))
 OR EXISTS(SELECT 1 FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING(set_group_id)
  JOIN logiqx_releases AS owner USING(set_id)
  WHERE groups.snapshot_key=NEW.snapshot_key AND NOT EXISTS(
   SELECT 1 FROM logiqx_games AS game JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key
   JOIN catalogs USING(catalog_key) JOIN publishing_sources USING(source_key)
   JOIN documents USING(document_key) JOIN parser_interpretations AS parser USING(interpretation_key)
   WHERE game.set_id=owner.set_id AND snapshot.snapshot_key=groups.snapshot_key
   AND parser.format='logiqx' AND groups.kind='root' AND sets.source_element_kind='logiqx_game'))
 OR EXISTS(SELECT 1 FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING(set_group_id)
  JOIN logiqx_bios_sets AS owner USING(set_id)
  WHERE groups.snapshot_key=NEW.snapshot_key AND NOT EXISTS(
   SELECT 1 FROM logiqx_games AS game JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key
   JOIN catalogs USING(catalog_key) JOIN publishing_sources USING(source_key)
   JOIN documents USING(document_key) JOIN parser_interpretations AS parser USING(interpretation_key)
   WHERE game.set_id=owner.set_id AND snapshot.snapshot_key=groups.snapshot_key
   AND parser.format='logiqx' AND groups.kind='root' AND sets.source_element_kind='logiqx_game'))
 OR EXISTS(SELECT 1 FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING(set_group_id)
  JOIN logiqx_archive_references AS owner USING(set_id)
  WHERE groups.snapshot_key=NEW.snapshot_key AND NOT EXISTS(
   SELECT 1 FROM logiqx_games AS game JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key
   JOIN catalogs USING(catalog_key) JOIN publishing_sources USING(source_key)
   JOIN documents USING(document_key) JOIN parser_interpretations AS parser USING(interpretation_key)
   WHERE game.set_id=owner.set_id AND snapshot.snapshot_key=groups.snapshot_key
   AND parser.format='logiqx' AND groups.kind='root' AND sets.source_element_kind='logiqx_game'))
 OR EXISTS(SELECT 1 FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING(set_group_id)
  JOIN logiqx_device_references AS owner USING(set_id)
  WHERE groups.snapshot_key=NEW.snapshot_key AND NOT EXISTS(
   SELECT 1 FROM logiqx_games AS game JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key
   JOIN catalogs USING(catalog_key) JOIN publishing_sources USING(source_key)
   JOIN documents USING(document_key) JOIN parser_interpretations AS parser USING(interpretation_key)
   WHERE game.set_id=owner.set_id AND snapshot.snapshot_key=groups.snapshot_key
   AND parser.format='logiqx' AND groups.kind='root' AND sets.source_element_kind='logiqx_game'))
 OR EXISTS(SELECT 1 FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING(set_group_id)
  JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id
  WHERE groups.snapshot_key=NEW.snapshot_key AND (
   (occurrence.claim_kind='logiqx_rom' AND (NOT EXISTS(SELECT 1 FROM logiqx_rom_claims WHERE occurrence_id=occurrence.occurrence_id)
      OR NOT EXISTS(SELECT 1 FROM logiqx_games WHERE set_id=sets.set_id)
      OR sets.source_element_kind<>'logiqx_game' OR groups.kind<>'root'
      OR NOT EXISTS(SELECT 1 FROM catalog_snapshots AS snapshot JOIN catalogs USING(catalog_key)
       JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
       JOIN parser_interpretations AS parser USING(interpretation_key)
       WHERE snapshot.snapshot_key=groups.snapshot_key AND parser.format='logiqx')))
   OR (occurrence.claim_kind='logiqx_disk' AND (NOT EXISTS(SELECT 1 FROM logiqx_disk_claims WHERE occurrence_id=occurrence.occurrence_id)
      OR NOT EXISTS(SELECT 1 FROM logiqx_games WHERE set_id=sets.set_id)
      OR sets.source_element_kind<>'logiqx_game' OR groups.kind<>'root'
      OR NOT EXISTS(SELECT 1 FROM catalog_snapshots AS snapshot JOIN catalogs USING(catalog_key)
       JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
       JOIN parser_interpretations AS parser USING(interpretation_key)
       WHERE snapshot.snapshot_key=groups.snapshot_key AND parser.format='logiqx')))
   OR (occurrence.claim_kind='logiqx_sample' AND (NOT EXISTS(SELECT 1 FROM logiqx_sample_claims WHERE occurrence_id=occurrence.occurrence_id)
      OR NOT EXISTS(SELECT 1 FROM logiqx_games WHERE set_id=sets.set_id)
      OR sets.source_element_kind<>'logiqx_game' OR groups.kind<>'root'
      OR NOT EXISTS(SELECT 1 FROM catalog_snapshots AS snapshot JOIN catalogs USING(catalog_key)
       JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
       JOIN parser_interpretations AS parser USING(interpretation_key)
       WHERE snapshot.snapshot_key=groups.snapshot_key AND parser.format='logiqx')))))
 -- Check the actual native values too: a corrupted occurrence kind must not
 -- hide an unpositioned value behind another otherwise valid media payload.
 OR EXISTS(SELECT 1 FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING(set_group_id)
  JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id
  JOIN logiqx_rom_claims AS owner USING(occurrence_id)
  WHERE groups.snapshot_key=NEW.snapshot_key AND (
   owner.occurrence_id<=0 OR occurrence.claim_kind<>'logiqx_rom'
   OR NOT EXISTS(SELECT 1 FROM logiqx_games WHERE set_id=sets.set_id)
   OR sets.source_element_kind<>'logiqx_game' OR groups.kind<>'root'
   OR NOT EXISTS(SELECT 1 FROM catalog_snapshots AS snapshot JOIN catalogs USING(catalog_key)
    JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
    JOIN parser_interpretations AS parser USING(interpretation_key)
    WHERE snapshot.snapshot_key=groups.snapshot_key AND parser.format='logiqx')))
 OR EXISTS(SELECT 1 FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING(set_group_id)
  JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id
  JOIN logiqx_disk_claims AS owner USING(occurrence_id)
  WHERE groups.snapshot_key=NEW.snapshot_key AND (
   owner.occurrence_id<=0 OR occurrence.claim_kind<>'logiqx_disk'
   OR NOT EXISTS(SELECT 1 FROM logiqx_games WHERE set_id=sets.set_id)
   OR sets.source_element_kind<>'logiqx_game' OR groups.kind<>'root'
   OR NOT EXISTS(SELECT 1 FROM catalog_snapshots AS snapshot JOIN catalogs USING(catalog_key)
    JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
    JOIN parser_interpretations AS parser USING(interpretation_key)
    WHERE snapshot.snapshot_key=groups.snapshot_key AND parser.format='logiqx')))
 OR EXISTS(SELECT 1 FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING(set_group_id)
  JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id
  JOIN logiqx_sample_claims AS owner USING(occurrence_id)
  WHERE groups.snapshot_key=NEW.snapshot_key AND (
   owner.occurrence_id<=0 OR occurrence.claim_kind<>'logiqx_sample'
   OR NOT EXISTS(SELECT 1 FROM logiqx_games WHERE set_id=sets.set_id)
   OR sets.source_element_kind<>'logiqx_game' OR groups.kind<>'root'
   OR NOT EXISTS(SELECT 1 FROM catalog_snapshots AS snapshot JOIN catalogs USING(catalog_key)
    JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
    JOIN parser_interpretations AS parser USING(interpretation_key)
    WHERE snapshot.snapshot_key=groups.snapshot_key AND parser.format='logiqx')))
BEGIN SELECT RAISE(ABORT,'Logiqx declared attributes require exact position closure'); END;

-- Document facts lacked the shared owner-table guards present on other
-- Logiqx owners; enforce the complete snapshot/parser ancestry here.
CREATE TRIGGER logiqx_document_facts_owner_insert
BEFORE INSERT ON logiqx_document_facts
WHEN EXISTS(SELECT 1 FROM logiqx_document_facts WHERE snapshot_key=NEW.snapshot_key)
 OR NOT EXISTS(SELECT 1 FROM catalog_snapshots AS snapshot
               JOIN catalogs USING(catalog_key) JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
               JOIN parser_interpretations AS parser USING(interpretation_key)
               WHERE snapshot.snapshot_key=NEW.snapshot_key AND parser.format='logiqx'
                 AND NOT EXISTS(SELECT 1 FROM snapshot_publications WHERE snapshot_key=snapshot.snapshot_key))
BEGIN SELECT RAISE(ABORT,'Logiqx document facts require a fresh unpublished Logiqx snapshot'); END;
CREATE TRIGGER logiqx_document_facts_immutable_update BEFORE UPDATE ON logiqx_document_facts
BEGIN SELECT RAISE(ABORT,'Logiqx document facts are immutable'); END;
CREATE TRIGGER logiqx_document_facts_immutable_delete BEFORE DELETE ON logiqx_document_facts
BEGIN SELECT RAISE(ABORT,'Logiqx document facts are immutable'); END;

-- Native value owners need their real ancestry even when foreign keys are off.
-- Game rows intentionally require no document-facts row: imports write games
-- before root facts are inserted in the same transaction.
CREATE TRIGGER logiqx_clrmamepro_options_attribute_parent_insert_guard
BEFORE INSERT ON logiqx_clrmamepro_options
WHEN NOT EXISTS(SELECT 1 FROM logiqx_document_facts AS facts
 JOIN catalog_snapshots AS snapshot USING(snapshot_key) JOIN catalogs USING(catalog_key)
 JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE facts.snapshot_key=NEW.snapshot_key AND parser.format='logiqx'
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications WHERE snapshot_key=snapshot.snapshot_key))
BEGIN SELECT RAISE(ABORT,'Logiqx ClrMamePro options require actual document facts and ancestry'); END;
CREATE TRIGGER logiqx_romcenter_options_attribute_parent_insert_guard
BEFORE INSERT ON logiqx_romcenter_options
WHEN NOT EXISTS(SELECT 1 FROM logiqx_document_facts AS facts
 JOIN catalog_snapshots AS snapshot USING(snapshot_key) JOIN catalogs USING(catalog_key)
 JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE facts.snapshot_key=NEW.snapshot_key AND parser.format='logiqx'
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications WHERE snapshot_key=snapshot.snapshot_key))
BEGIN SELECT RAISE(ABORT,'Logiqx RomCenter options require actual document facts and ancestry'); END;

CREATE TRIGGER logiqx_games_attribute_parent_insert_guard
BEFORE INSERT ON logiqx_games
WHEN NOT EXISTS(SELECT 1 FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN catalog_snapshots AS snapshot USING(snapshot_key) JOIN catalogs USING(catalog_key)
 JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE sets.set_id=NEW.set_id AND sets.source_element_kind='logiqx_game'
 AND groups.kind='root' AND parser.format='logiqx'
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications WHERE snapshot_key=snapshot.snapshot_key))
BEGIN SELECT RAISE(ABORT,'Logiqx game details require a root set with actual Logiqx ancestry'); END;

CREATE TRIGGER logiqx_releases_actual_game_owner_insert_guard
BEFORE INSERT ON logiqx_releases
WHEN NOT EXISTS(SELECT 1 FROM logiqx_games AS game JOIN catalog_sets AS sets USING(set_id)
 JOIN catalog_set_groups AS groups USING(set_group_id) JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN catalogs USING(catalog_key) JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE game.set_id=NEW.set_id AND sets.source_element_kind='logiqx_game'
 AND groups.kind='root' AND parser.format='logiqx'
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications WHERE snapshot_key=snapshot.snapshot_key))
BEGIN SELECT RAISE(ABORT,'Logiqx release requires an actual game with complete ancestry'); END;
CREATE TRIGGER logiqx_bios_actual_game_owner_insert_guard
BEFORE INSERT ON logiqx_bios_sets
WHEN NOT EXISTS(SELECT 1 FROM logiqx_games AS game JOIN catalog_sets AS sets USING(set_id)
 JOIN catalog_set_groups AS groups USING(set_group_id) JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN catalogs USING(catalog_key) JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE game.set_id=NEW.set_id AND sets.source_element_kind='logiqx_game'
 AND groups.kind='root' AND parser.format='logiqx'
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications WHERE snapshot_key=snapshot.snapshot_key))
BEGIN SELECT RAISE(ABORT,'Logiqx BIOS set requires an actual game with complete ancestry'); END;
CREATE TRIGGER logiqx_archive_actual_game_owner_insert_guard
BEFORE INSERT ON logiqx_archive_references
WHEN NOT EXISTS(SELECT 1 FROM logiqx_games AS game JOIN catalog_sets AS sets USING(set_id)
 JOIN catalog_set_groups AS groups USING(set_group_id) JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN catalogs USING(catalog_key) JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE game.set_id=NEW.set_id AND sets.source_element_kind='logiqx_game'
 AND groups.kind='root' AND parser.format='logiqx'
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications WHERE snapshot_key=snapshot.snapshot_key))
BEGIN SELECT RAISE(ABORT,'Logiqx archive reference requires an actual game with complete ancestry'); END;
CREATE TRIGGER logiqx_device_actual_game_owner_insert_guard
BEFORE INSERT ON logiqx_device_references
WHEN NOT EXISTS(SELECT 1 FROM logiqx_games AS game JOIN catalog_sets AS sets USING(set_id)
 JOIN catalog_set_groups AS groups USING(set_group_id) JOIN catalog_snapshots AS snapshot USING(snapshot_key)
 JOIN catalogs USING(catalog_key) JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE game.set_id=NEW.set_id AND sets.source_element_kind='logiqx_game'
 AND groups.kind='root' AND parser.format='logiqx'
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications WHERE snapshot_key=snapshot.snapshot_key))
BEGIN SELECT RAISE(ABORT,'Logiqx device reference requires an actual game with complete ancestry'); END;

CREATE TRIGGER logiqx_rom_claims_actual_game_owner_insert_guard
BEFORE INSERT ON logiqx_rom_claims
-- SQLite exposes an automatic rowid as -1 before allocation, not NULL.
WHEN NEW.occurrence_id<=0 OR NOT EXISTS(
 SELECT 1 FROM asset_occurrences AS occurrence JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
 JOIN logiqx_games AS game USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN catalog_snapshots AS snapshot USING(snapshot_key) JOIN catalogs USING(catalog_key)
 JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE occurrence.occurrence_id=NEW.occurrence_id AND occurrence.claim_kind='logiqx_rom'
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND parser.format='logiqx'
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications WHERE snapshot_key=snapshot.snapshot_key))
BEGIN SELECT RAISE(ABORT,'Logiqx ROM claim requires an actual matching occurrence and game ancestry'); END;
CREATE TRIGGER logiqx_disk_claims_actual_game_owner_insert_guard
BEFORE INSERT ON logiqx_disk_claims
WHEN NEW.occurrence_id<=0 OR NOT EXISTS(
 SELECT 1 FROM asset_occurrences AS occurrence JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
 JOIN logiqx_games AS game USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN catalog_snapshots AS snapshot USING(snapshot_key) JOIN catalogs USING(catalog_key)
 JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE occurrence.occurrence_id=NEW.occurrence_id AND occurrence.claim_kind='logiqx_disk'
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND parser.format='logiqx'
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications WHERE snapshot_key=snapshot.snapshot_key))
BEGIN SELECT RAISE(ABORT,'Logiqx disk claim requires an actual matching occurrence and game ancestry'); END;
CREATE TRIGGER logiqx_sample_claims_actual_game_owner_insert_guard
BEFORE INSERT ON logiqx_sample_claims
WHEN NEW.occurrence_id<=0 OR NOT EXISTS(
 SELECT 1 FROM asset_occurrences AS occurrence JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
 JOIN logiqx_games AS game USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id)
 JOIN catalog_snapshots AS snapshot USING(snapshot_key) JOIN catalogs USING(catalog_key)
 JOIN publishing_sources USING(source_key) JOIN documents USING(document_key)
 JOIN parser_interpretations AS parser USING(interpretation_key)
 WHERE occurrence.occurrence_id=NEW.occurrence_id AND occurrence.claim_kind='logiqx_sample'
 AND sets.source_element_kind='logiqx_game' AND groups.kind='root' AND parser.format='logiqx'
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications WHERE snapshot_key=snapshot.snapshot_key))
BEGIN SELECT RAISE(ABORT,'Logiqx sample claim requires an actual matching occurrence and game ancestry'); END;

-- BEFORE INSERT collision checks close SQLite REPLACE's implicit-delete path
-- even when recursive_triggers is disabled. Existing UPDATE/DELETE immutability
-- triggers continue to protect ordinary DML.
CREATE TRIGGER logiqx_clrmamepro_options_no_replace BEFORE INSERT ON logiqx_clrmamepro_options
WHEN EXISTS(SELECT 1 FROM logiqx_clrmamepro_options WHERE snapshot_key=NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT,'Logiqx clrmamepro options are immutable'); END;
CREATE TRIGGER logiqx_romcenter_options_no_replace BEFORE INSERT ON logiqx_romcenter_options
WHEN EXISTS(SELECT 1 FROM logiqx_romcenter_options WHERE snapshot_key=NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT,'Logiqx romcenter options are immutable'); END;
CREATE TRIGGER logiqx_games_no_replace BEFORE INSERT ON logiqx_games
WHEN EXISTS(SELECT 1 FROM logiqx_games WHERE set_id=NEW.set_id)
BEGIN SELECT RAISE(ABORT,'Logiqx game details are immutable'); END;
CREATE TRIGGER logiqx_releases_no_replace BEFORE INSERT ON logiqx_releases
WHEN EXISTS(SELECT 1 FROM logiqx_releases WHERE (set_id=NEW.set_id AND release_order=NEW.release_order)
 OR (set_id=NEW.set_id AND source_order=NEW.source_order))
BEGIN SELECT RAISE(ABORT,'Logiqx release details are immutable'); END;
CREATE TRIGGER logiqx_bios_sets_no_replace BEFORE INSERT ON logiqx_bios_sets
WHEN EXISTS(SELECT 1 FROM logiqx_bios_sets WHERE (set_id=NEW.set_id AND bios_order=NEW.bios_order)
 OR (set_id=NEW.set_id AND source_order=NEW.source_order))
BEGIN SELECT RAISE(ABORT,'Logiqx BIOS details are immutable'); END;
CREATE TRIGGER logiqx_archive_references_no_replace BEFORE INSERT ON logiqx_archive_references
WHEN EXISTS(SELECT 1 FROM logiqx_archive_references WHERE (set_id=NEW.set_id AND archive_order=NEW.archive_order)
 OR (set_id=NEW.set_id AND source_order=NEW.source_order))
BEGIN SELECT RAISE(ABORT,'Logiqx archive references are immutable'); END;
CREATE TRIGGER logiqx_rom_claims_no_replace BEFORE INSERT ON logiqx_rom_claims
WHEN EXISTS(SELECT 1 FROM logiqx_rom_claims WHERE occurrence_id=NEW.occurrence_id)
BEGIN SELECT RAISE(ABORT,'Logiqx ROM claims are immutable'); END;
CREATE TRIGGER logiqx_disk_claims_no_replace BEFORE INSERT ON logiqx_disk_claims
WHEN EXISTS(SELECT 1 FROM logiqx_disk_claims WHERE occurrence_id=NEW.occurrence_id)
BEGIN SELECT RAISE(ABORT,'Logiqx disk claims are immutable'); END;
CREATE TRIGGER logiqx_sample_claims_no_replace BEFORE INSERT ON logiqx_sample_claims
WHEN EXISTS(SELECT 1 FROM logiqx_sample_claims WHERE occurrence_id=NEW.occurrence_id)
BEGIN SELECT RAISE(ABORT,'Logiqx sample claims are immutable'); END;
