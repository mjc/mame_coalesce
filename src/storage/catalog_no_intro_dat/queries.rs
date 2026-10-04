//! Indexed SQL for bounded native flat-DAT queries.

pub(super) const SNAPSHOT: &str = "
SELECT snapshots.snapshot_key, sources.source_key, sources.display_name AS source_name,
       catalogs.catalog_key, catalogs.display_name AS catalog_name, snapshots.document_key,
       snapshots.interpretation_key, interpretations.format, header.version_text AS declared_version,
       publication.snapshot_key IS NOT NULL AS published,
       typeof(snapshots.snapshot_key)='text' AND typeof(snapshots.document_key)='text'
         AND typeof(snapshots.interpretation_key)='text'
         AND typeof(catalogs.catalog_key)='text' AND typeof(catalogs.display_name)='text'
         AND typeof(sources.source_key)='text' AND typeof(sources.display_name)='text'
         AND typeof(documents.document_key)='text'
         AND typeof(interpretations.interpretation_key)='text'
         AND typeof(interpretations.format)='text'
         AND typeof(publication.snapshot_key)='text' AS valid
FROM catalog_snapshots AS snapshots
JOIN catalogs ON catalogs.catalog_key=snapshots.catalog_key
JOIN publishing_sources AS sources ON sources.source_key=catalogs.source_key
JOIN documents ON documents.document_key=snapshots.document_key
JOIN parser_interpretations AS interpretations
  ON interpretations.interpretation_key=snapshots.interpretation_key
LEFT JOIN snapshot_publications AS publication
  ON publication.snapshot_key=snapshots.snapshot_key
 AND publication.catalog_key=snapshots.catalog_key
 AND publication.document_key=snapshots.document_key
 AND publication.interpretation_key=snapshots.interpretation_key
LEFT JOIN no_intro_dat_headers AS header ON header.snapshot_key=snapshots.snapshot_key
WHERE snapshots.snapshot_key=?";

pub(super) const COUNTS: &str = "
SELECT game_count, rom_count, category_count, identifier_count, release_count,
       clrmamepro_option_count, romcenter_option_count, header_field_count,
       clrmamepro_field_count, romcenter_field_count, game_field_count, rom_field_count,
       typeof(game_count)='integer' AND typeof(rom_count)='integer'
         AND typeof(category_count)='integer' AND typeof(identifier_count)='integer'
         AND typeof(release_count)='integer' AND typeof(clrmamepro_option_count)='integer'
         AND typeof(romcenter_option_count)='integer' AND typeof(header_field_count)='integer'
         AND typeof(clrmamepro_field_count)='integer' AND typeof(romcenter_field_count)='integer'
         AND typeof(game_field_count)='integer' AND typeof(rom_field_count)='integer' AS valid
FROM no_intro_dat_parse_counts WHERE snapshot_key=?";

pub(super) const DOCUMENT: &str = "
SELECT schema_location, source_line AS line, source_column AS column,
       typeof(snapshot_key)='text' AND typeof(source_line)='integer'
         AND typeof(source_column)='integer'
         AND (schema_location IS NULL OR typeof(schema_location)='text') AS valid
FROM no_intro_dat_documents WHERE snapshot_key=?";

pub(super) const HEADER: &str = "
SELECT source_order, source_line AS line, source_column AS column,
       id_text, name, description, version_text, date, author, homepage, url,
       trademarks, piracy, subset, comment,
       typeof(source_order)='integer' AND typeof(source_line)='integer'
         AND typeof(source_column)='integer'
         AND (id_text IS NULL OR typeof(id_text)='text')
         AND (name IS NULL OR typeof(name)='text')
         AND (description IS NULL OR typeof(description)='text')
         AND (version_text IS NULL OR typeof(version_text)='text')
         AND (date IS NULL OR typeof(date)='text') AND (author IS NULL OR typeof(author)='text')
         AND (homepage IS NULL OR typeof(homepage)='text') AND (url IS NULL OR typeof(url)='text')
         AND (trademarks IS NULL OR typeof(trademarks)='text')
         AND (piracy IS NULL OR typeof(piracy)='text') AND (subset IS NULL OR typeof(subset)='text')
         AND (comment IS NULL OR typeof(comment)='text') AS valid
FROM no_intro_dat_headers WHERE snapshot_key=?";

pub(super) const CLRMAMEPRO: &str = "
SELECT source_order, source_line AS line, source_column AS column,
       forcenodump_text, forcenodump_effective, header_text,
       typeof(source_order)='integer' AND typeof(source_line)='integer'
         AND typeof(source_column)='integer'
         AND (forcenodump_text IS NULL OR typeof(forcenodump_text)='text')
         AND (forcenodump_effective IS NULL OR typeof(forcenodump_effective)='text')
         AND (header_text IS NULL OR typeof(header_text)='text') AS valid
FROM no_intro_dat_clrmamepro_options WHERE snapshot_key=?";

pub(super) const ROMCENTER: &str = "
SELECT source_order, source_line AS line, source_column AS column, plugin_text,
       typeof(source_order)='integer' AND typeof(source_line)='integer'
         AND typeof(source_column)='integer'
         AND (plugin_text IS NULL OR typeof(plugin_text)='text') AS valid
FROM no_intro_dat_romcenter_options WHERE snapshot_key=?";

pub(super) const ROOT_GROUPS: &str = "
SELECT set_group_id, kind, list_order,
       typeof(set_group_id)='integer' AND typeof(snapshot_key)='text'
         AND typeof(kind)='text' AND typeof(list_order)='integer' AS valid
FROM catalog_set_groups WHERE snapshot_key=? ORDER BY kind, list_order, set_group_id LIMIT 3";

pub(super) fn games(after_cursor: bool) -> String {
    let continuation = if after_cursor {
        " AND (sets.list_order, sets.set_id) > (?, ?)"
    } else {
        ""
    };
    format!(
        "SELECT sets.set_id, sets.set_group_id, sets.set_name, sets.list_order, \
                sets.source_element_kind, sets.source_line AS line, sets.source_column AS column, \
                native.set_id AS native_id, native.source_element_kind AS native_kind, native.source_order, native.id_text, \
                native.description_text, \
                typeof(sets.set_id)='integer' AND typeof(sets.set_group_id)='integer' \
                  AND typeof(sets.list_order)='integer' AND typeof(sets.source_line)='integer' \
                  AND typeof(sets.source_column)='integer' AND typeof(sets.set_name)='text' \
                  AND typeof(sets.source_element_kind)='text' \
                  AND typeof(native.set_id)='integer' AND typeof(native.source_order)='integer' \
                  AND typeof(native.source_element_kind)='text' AND native.source_element_kind='no_intro_dat_game' \
                  AND (native.id_text IS NULL OR typeof(native.id_text)='text') \
                  AND (native.description_text IS NULL OR typeof(native.description_text)='text') AS valid \
         FROM catalog_sets AS sets LEFT JOIN no_intro_dat_games AS native USING(set_id) \
         WHERE sets.set_group_id=? AND sets.source_element_kind='no_intro_dat_game'{continuation} \
         ORDER BY sets.list_order, sets.set_id LIMIT ?"
    )
}

pub(super) const CURSOR_ANCHOR: &str = "
SELECT sets.set_id, sets.set_group_id, sets.set_name, sets.list_order,
       sets.source_element_kind, sets.source_line AS line, sets.source_column AS column,
       native.set_id AS native_id, native.source_element_kind AS native_kind,
       native.source_order, native.id_text, native.description_text,
       typeof(sets.set_id)='integer' AND typeof(sets.set_group_id)='integer'
         AND typeof(sets.list_order)='integer' AND typeof(sets.source_line)='integer'
         AND typeof(sets.source_column)='integer' AND typeof(sets.set_name)='text'
         AND typeof(sets.source_element_kind)='text'
         AND typeof(native.set_id)='integer'
         AND typeof(native.source_element_kind)='text' AND native.source_element_kind='no_intro_dat_game'
         AND typeof(native.source_order)='integer'
         AND (native.id_text IS NULL OR typeof(native.id_text)='text')
         AND (native.description_text IS NULL OR typeof(native.description_text)='text') AS valid
FROM catalog_sets AS sets LEFT JOIN no_intro_dat_games AS native USING(set_id)
WHERE sets.set_group_id=? AND sets.set_id=? AND sets.list_order=?";

pub(super) fn requested(count: usize) -> String {
    let values = std::iter::repeat_n("(?)", count)
        .collect::<Vec<_>>()
        .join(",");
    format!("WITH requested(owner_id) AS (VALUES {values}) ")
}

pub(super) fn position_rows(table: &str, owner_column: &str, count: usize) -> String {
    let domain = if table == "no_intro_dat_game_field_positions" {
        "native.position_domain"
    } else {
        "0"
    };
    let domain_valid = if table == "no_intro_dat_game_field_positions" {
        "AND typeof(native.position_domain)='integer'"
    } else {
        ""
    };
    format!(
        "{}SELECT native.{owner_column} AS owner_id, native.field_kind, {domain} AS position_domain, native.source_order, \
                native.source_line AS line, native.source_column AS column, \
                typeof(native.{owner_column})='integer' AND typeof(native.field_kind)='integer' \
                  {domain_valid} \
                  AND typeof(native.source_order)='integer' AND typeof(native.source_line)='integer' \
                  AND typeof(native.source_column)='integer' AS valid \
         FROM requested CROSS JOIN {table} AS native \
         WHERE native.{owner_column}=requested.owner_id ORDER BY native.{owner_column}, native.field_kind",
        requested(count)
    )
}

pub(super) fn category_rows(count: usize) -> String {
    format!(
        "{}SELECT native.set_id AS game_id, native.category_order AS family_order, \
                native.category AS value, native.source_order, native.source_line AS line, \
                native.source_column AS column, \
                typeof(native.set_id)='integer' AND typeof(native.category_order)='integer' \
                  AND typeof(native.source_order)='integer' AND typeof(native.source_line)='integer' \
                  AND typeof(native.source_column)='integer' \
                  AND typeof(native.category)='text' AS valid \
         FROM requested CROSS JOIN no_intro_dat_categories AS native \
         WHERE native.set_id=requested.owner_id ORDER BY native.set_id, native.source_order, native.category_order",
        requested(count)
    )
}

pub(super) fn identifier_rows(count: usize) -> String {
    format!(
        "{}SELECT native.set_id AS game_id, native.identifier_order AS family_order, \
                native.identifier AS value, native.source_order, native.source_line AS line, \
                native.source_column AS column, \
                typeof(native.set_id)='integer' AND typeof(native.identifier_order)='integer' \
                  AND typeof(native.source_order)='integer' AND typeof(native.source_line)='integer' \
                  AND typeof(native.source_column)='integer' \
                  AND typeof(native.identifier)='text' AS valid \
         FROM requested CROSS JOIN no_intro_dat_identifiers AS native \
         WHERE native.set_id=requested.owner_id ORDER BY native.set_id, native.source_order, native.identifier_order",
        requested(count)
    )
}

pub(super) fn release_rows(count: usize) -> String {
    format!(
        "{}SELECT native.set_id AS game_id, native.release_order AS family_order, native.name, \
                native.region, native.source_order, native.source_line AS line, native.source_column AS column, \
                native.name_source_order, native.name_source_line, native.name_source_column, \
                native.region_source_order, native.region_source_line, native.region_source_column, \
                typeof(native.set_id)='integer' AND typeof(native.release_order)='integer' \
                  AND typeof(native.source_order)='integer' AND typeof(native.source_line)='integer' \
                  AND typeof(native.source_column)='integer' AND typeof(native.name_source_order)='integer' \
                  AND typeof(native.name_source_line)='integer' AND typeof(native.name_source_column)='integer' \
                  AND typeof(native.region_source_order)='integer' AND typeof(native.region_source_line)='integer' \
                  AND typeof(native.region_source_column)='integer' \
                  AND typeof(native.name)='text' AND typeof(native.region)='text' AS valid \
         FROM requested CROSS JOIN no_intro_dat_releases AS native \
         WHERE native.set_id=requested.owner_id ORDER BY native.set_id, native.source_order, native.release_order",
        requested(count)
    )
}

pub(super) fn rom_rows(count: usize) -> String {
    format!(
        "{}SELECT occurrence.occurrence_id, occurrence.occurrence_order, occurrence.record_id AS game_id, \
                claim.source_order, claim.source_line AS line, claim.source_column AS column, \
                claim.name, claim.evidence_scope, claim.evidence_provenance, \
                typeof(occurrence.occurrence_id)='integer' AND typeof(occurrence.occurrence_order)='integer' \
                  AND typeof(occurrence.record_id)='integer' AND typeof(claim.occurrence_id)='integer' \
                  AND typeof(claim.source_order)='integer' AND typeof(claim.source_line)='integer' \
                  AND typeof(claim.source_column)='integer' AND typeof(claim.name)='text' \
                  AND typeof(claim.evidence_scope)='text' \
                  AND typeof(claim.evidence_provenance)='text' AND claim.evidence_provenance='source_declared' \
                  AND occurrence.claim_kind='no_intro_dat_rom' AND typeof(sets.set_id)='integer' \
                  AND sets.source_element_kind='no_intro_dat_game' \
                  AND typeof(native_game.set_id)='integer' AND native_game.set_id=sets.set_id \
                  AND typeof(groups.set_group_id)='integer' AND groups.kind='root' \
                  AND groups.list_order=0 AND groups.snapshot_key=? \
                  AND interpretation.format IN ('no-intro-dat-v3-strict','no-intro-dat-v3-compatible', \
                    'no-intro-dat-v4-strict','no-intro-dat-v4-compatible') AS valid \
         FROM requested CROSS JOIN asset_occurrences AS occurrence \
         LEFT JOIN no_intro_dat_rom_claims AS claim USING(occurrence_id) \
         JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id \
         JOIN no_intro_dat_games AS native_game ON native_game.set_id=sets.set_id \
         JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id \
         JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key \
         JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key \
         WHERE sets.set_id=requested.owner_id \
         ORDER BY sets.list_order, occurrence.occurrence_order, occurrence.occurrence_id",
        requested(count)
    )
}

pub(super) fn parent_rows(count: usize) -> String {
    format!(
        "{}SELECT link.set_id AS game_id, link.link_kind, link.target_literal, link.relationship_id, \
                reported.source_reference_kind, identity.origin, identity.snapshot_key, \
                typeof(link.set_id)='integer' AND typeof(link.relationship_id)='integer' \
                  AND typeof(link.link_kind)='text' \
                  AND typeof(link.target_literal)='text' \
                  AND typeof(reported.relationship_id)='integer' \
                  AND typeof(reported.source_reference_kind)='text' \
                  AND typeof(identity.relationship_id)='integer' \
                  AND typeof(identity.origin)='text' AND typeof(identity.snapshot_key)='text' AS valid \
         FROM requested CROSS JOIN no_intro_dat_set_links AS link \
         LEFT JOIN reported_catalog_relationships AS reported USING(relationship_id) \
         LEFT JOIN catalog_relationships AS identity USING(relationship_id) \
         WHERE link.set_id=requested.owner_id ORDER BY link.set_id, link.link_kind",
        requested(count)
    )
}

pub(super) fn mixed_child_rows(count: usize) -> String {
    format!(
        "{}SELECT native.set_id AS game_id, 'category' AS child_kind, native.source_order, \
                typeof(native.set_id)='integer' AND typeof(native.source_order)='integer' AS valid \
         FROM requested CROSS JOIN no_intro_dat_categories AS native WHERE native.set_id=requested.owner_id \
         UNION ALL SELECT native.set_id, 'identifier', native.source_order, \
                typeof(native.set_id)='integer' AND typeof(native.source_order)='integer' \
         FROM requested CROSS JOIN no_intro_dat_identifiers AS native WHERE native.set_id=requested.owner_id \
         UNION ALL SELECT native.set_id, 'release', native.source_order, \
                typeof(native.set_id)='integer' AND typeof(native.source_order)='integer' \
         FROM requested CROSS JOIN no_intro_dat_releases AS native WHERE native.set_id=requested.owner_id \
         UNION ALL SELECT occurrence.record_id, 'rom', native.source_order, \
                typeof(occurrence.record_id)='integer' AND typeof(native.source_order)='integer' \
         FROM requested CROSS JOIN asset_occurrences AS occurrence \
         JOIN no_intro_dat_rom_claims AS native USING(occurrence_id) \
         WHERE occurrence.record_id=requested.owner_id \
         UNION ALL SELECT native.set_id, 'description', position.source_order, \
                typeof(native.set_id)='integer' AND typeof(position.source_order)='integer' \
         FROM requested CROSS JOIN no_intro_dat_games AS native \
         JOIN no_intro_dat_game_field_positions AS position ON position.set_id=native.set_id \
         WHERE native.set_id=requested.owner_id AND position.field_kind=4 \
         ORDER BY game_id, source_order, child_kind",
        requested(count)
    )
}
