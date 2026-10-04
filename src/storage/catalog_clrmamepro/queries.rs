//! Production SQL uses actual snapshot, set and occurrence owner keys.

pub(super) const SNAPSHOT: &str = "
SELECT snapshots.snapshot_key, sources.source_key, sources.display_name AS source_name,
       catalogs.catalog_key, catalogs.display_name AS catalog_name, snapshots.document_key,
       snapshots.interpretation_key, interpretations.format, interpretations.rules_version,
       header.version AS declared_version,
       typeof(snapshots.snapshot_key)='text' AND typeof(snapshots.document_key)='text'
         AND typeof(snapshots.interpretation_key)='text' AND typeof(catalogs.catalog_key)='text'
         AND typeof(catalogs.display_name)='text' AND typeof(sources.source_key)='text'
         AND typeof(sources.display_name)='text' AND typeof(documents.document_key)='text'
         AND typeof(interpretations.format)='text' AND typeof(interpretations.rules_version)='text'
         AND (header.version IS NULL OR typeof(header.version)='text') AS valid
FROM catalog_snapshots AS snapshots
JOIN snapshot_publications AS publication ON publication.snapshot_key=snapshots.snapshot_key
 AND publication.catalog_key=snapshots.catalog_key AND publication.document_key=snapshots.document_key
 AND publication.interpretation_key=snapshots.interpretation_key
JOIN catalogs ON catalogs.catalog_key=snapshots.catalog_key
JOIN publishing_sources AS sources ON sources.source_key=catalogs.source_key
JOIN documents ON documents.document_key=snapshots.document_key
JOIN parser_interpretations AS interpretations ON interpretations.interpretation_key=snapshots.interpretation_key
LEFT JOIN cmp_header_facts AS header ON header.snapshot_key=snapshots.snapshot_key
WHERE snapshots.snapshot_key=?";

pub(super) const GROUPS: &str = "
SELECT set_group_id,kind,list_order,
 typeof(set_group_id)='integer' AND typeof(kind)='text' AND typeof(list_order)='integer'
 AND typeof(snapshot_key)='text' AS valid
FROM catalog_set_groups WHERE snapshot_key=? ORDER BY kind,list_order,set_group_id LIMIT 3";

fn set_select(predicate: &str) -> String {
    format!(
        "SELECT sets.set_id,sets.set_group_id,sets.set_name,sets.list_order,
       sets.source_element_kind,sets.source_line AS line,sets.source_column AS column,
       native.record_id AS native_id,native.source_block,native.document_order,
       native.description,native.year,native.manufacturer,native.rebuildto,native.region,
       native.release_year_text,native.release_month_text,native.release_day_text,native.serial,
       typeof(sets.set_id)='integer' AND typeof(sets.set_group_id)='integer'
         AND typeof(sets.set_name)='text' AND typeof(sets.list_order)='integer'
         AND typeof(sets.source_element_kind)='text' AND typeof(sets.source_line)='integer'
         AND typeof(sets.source_column)='integer' AND typeof(native.record_id)='integer'
         AND typeof(native.source_block)='text' AND typeof(native.document_order)='integer'
         AND (native.description IS NULL OR typeof(native.description)='text')
         AND (native.year IS NULL OR typeof(native.year)='text')
         AND (native.manufacturer IS NULL OR typeof(native.manufacturer)='text')
         AND (native.rebuildto IS NULL OR typeof(native.rebuildto)='text')
         AND (native.region IS NULL OR typeof(native.region)='text')
         AND (native.release_year_text IS NULL OR typeof(native.release_year_text)='text')
         AND (native.release_month_text IS NULL OR typeof(native.release_month_text)='text')
         AND (native.release_day_text IS NULL OR typeof(native.release_day_text)='text')
         AND (native.serial IS NULL OR typeof(native.serial)='text') AS valid
FROM catalog_sets AS sets LEFT JOIN cmp_set_facts AS native ON native.record_id=sets.set_id
WHERE {predicate}"
    )
}

pub(super) fn sets(continuation: bool) -> String {
    let after = if continuation {
        " AND (sets.list_order,sets.set_id)>(?,?)"
    } else {
        ""
    };
    format!(
        "{} ORDER BY sets.list_order,sets.set_id LIMIT ?",
        set_select(&format!("sets.set_group_id=?{after}"))
    )
}

pub(super) fn anchor() -> String {
    set_select("sets.set_group_id=? AND sets.set_id=? AND sets.list_order=?")
}

pub(super) fn requested(count: usize) -> String {
    let values = std::iter::repeat_n("(?)", count)
        .collect::<Vec<_>>()
        .join(",");
    format!("WITH requested(owner_id) AS (VALUES {values}) ")
}

pub(super) fn set_positions(count: usize) -> String {
    format!("{}SELECT native.record_id AS owner_id,native.field_kind,native.source_field,
       native.source_order,native.is_quoted,native.source_line AS line,native.source_column AS column,
       typeof(native.record_id)='integer' AND typeof(native.field_kind)='integer'
         AND typeof(native.source_field)='text' AND typeof(native.source_order)='integer'
         AND typeof(native.is_quoted)='integer' AND typeof(native.source_line)='integer'
         AND typeof(native.source_column)='integer' AS valid
FROM requested CROSS JOIN cmp_set_field_positions AS native
WHERE native.record_id=requested.owner_id ORDER BY native.record_id,native.source_order",requested(count))
}

pub(super) fn parents(count: usize) -> String {
    format!("{}SELECT native.set_id AS owner_id,native.link_kind,native.target_name,native.relationship_id,
       registry.snapshot_key,registry.origin,reported.source_reference_kind,
       typeof(native.set_id)='integer' AND typeof(native.link_kind)='text'
         AND typeof(native.target_name)='text' AND typeof(native.relationship_id)='integer'
         AND typeof(native.source_reference_kind)='text'
         AND typeof(registry.snapshot_key)='text' AND typeof(registry.origin)='text'
         AND typeof(reported.source_reference_kind)='text' AS valid
FROM requested CROSS JOIN clrmamepro_set_links AS native
LEFT JOIN catalog_relationships AS registry USING(relationship_id)
LEFT JOIN reported_catalog_relationships AS reported USING(relationship_id)
WHERE native.set_id=requested.owner_id ORDER BY native.set_id,native.link_kind",requested(count))
}

pub(super) fn media(count: usize) -> String {
    format!("{}SELECT occurrence.record_id AS owner_id,occurrence.occurrence_id,
       occurrence.occurrence_order,occurrence.claim_kind,
       typeof(occurrence.record_id)='integer' AND typeof(occurrence.occurrence_id)='integer'
         AND typeof(occurrence.occurrence_order)='integer' AND typeof(occurrence.claim_kind)='text' AS valid
FROM requested CROSS JOIN asset_occurrences AS occurrence
WHERE occurrence.record_id=requested.owner_id
ORDER BY occurrence.record_id,occurrence.occurrence_order,occurrence.occurrence_id",requested(count))
}
