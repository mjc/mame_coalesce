//! Statement-local requested owners remain the outer loop of every payload query.
use super::OccurrenceId;

pub(super) fn requested(ids: &[OccurrenceId]) -> String {
    let values = ids
        .iter()
        .map(|id| format!("({})", id.database_value()))
        .collect::<Vec<_>>()
        .join(",");
    format!("WITH requested(occurrence_id) AS (VALUES {values}) ")
}

pub(super) fn owners() -> String {
    format!(
        "SELECT requested.occurrence_id, occurrence.claim_kind,
                rom.occurrence_id AS rom_id, sample.occurrence_id AS sample_id,
                occurrence.content_uuid,
                COALESCE(({}),0) AS valid_owner,
                COALESCE(({}),0) AS valid_identity
         FROM requested
         LEFT JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=requested.occurrence_id
         LEFT JOIN cmp_rom_claims AS rom ON rom.occurrence_id=requested.occurrence_id
         LEFT JOIN cmp_samples AS sample ON sample.occurrence_id=requested.occurrence_id
         LEFT JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
         LEFT JOIN cmp_set_facts AS native ON native.record_id=sets.set_id
         LEFT JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id
         LEFT JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key
         LEFT JOIN cmp_documents AS document ON document.snapshot_key=snapshot.snapshot_key
         LEFT JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
         LEFT JOIN snapshot_publications AS publication ON publication.snapshot_key=snapshot.snapshot_key
         LEFT JOIN catalog_contents AS issued ON issued.content_uuid=occurrence.content_uuid
         WHERE occurrence.claim_kind IN ('cmp_rom','cmp_sample') OR rom.occurrence_id IS NOT NULL
            OR sample.occurrence_id IS NOT NULL
            OR EXISTS (SELECT 1 FROM cmp_rom_field_positions AS position WHERE position.occurrence_id=requested.occurrence_id)
            OR EXISTS (SELECT 1 FROM cmp_set_rom_positions AS form WHERE form.occurrence_id=requested.occurrence_id)
            OR EXISTS (SELECT 1 FROM clrmamepro_rom_merges AS declaration WHERE declaration.occurrence_id=requested.occurrence_id)
         ORDER BY requested.occurrence_id",
        owner_check(), identity_check()
    )
}

const fn owner_check() -> &'static str {
    "typeof(occurrence.occurrence_id)='integer' AND occurrence.occurrence_id>0
     AND typeof(occurrence.record_id)='integer' AND occurrence.record_id>0
     AND typeof(occurrence.occurrence_order)='integer' AND occurrence.occurrence_order>=0
     AND typeof(occurrence.claim_kind)='text' AND occurrence.claim_kind IN ('cmp_rom','cmp_sample')
     AND NOT EXISTS (SELECT 1 FROM cmp_set_rom_positions AS sample_form
         WHERE sample_form.occurrence_id=occurrence.occurrence_id
           AND occurrence.claim_kind='cmp_sample')
     AND typeof(sets.set_id)='integer' AND sets.set_id>0
     AND typeof(sets.set_group_id)='integer' AND sets.set_group_id>0
     AND typeof(sets.source_element_kind)='text' AND sets.source_element_kind='cmp_set'
     AND typeof(sets.list_order)='integer' AND sets.list_order>=0 AND typeof(sets.set_name)='text'
     AND typeof(sets.source_line)='integer' AND sets.source_line>0
     AND typeof(sets.source_column)='integer' AND sets.source_column>0
     AND typeof(native.record_id)='integer' AND native.record_id=sets.set_id
     AND typeof(native.source_block)='text' AND lower(native.source_block) IN ('set','game')
     AND typeof(native.document_order)='integer' AND native.document_order>=0
     AND typeof(groups.set_group_id)='integer' AND groups.set_group_id>0
     AND typeof(groups.kind)='text' AND groups.kind='root'
     AND typeof(groups.list_order)='integer' AND groups.list_order=0
     AND typeof(groups.snapshot_key)='text' AND typeof(snapshot.snapshot_key)='text'
     AND typeof(snapshot.catalog_key)='text' AND typeof(snapshot.document_key)='text'
     AND typeof(snapshot.interpretation_key)='text'
     AND typeof(document.snapshot_key)='text'
     AND typeof(document.header_present)='integer' AND document.header_present IN (0,1)
     AND typeof(document.comment_count)='integer' AND document.comment_count>=0
     AND typeof(interpretation.format)='text' AND interpretation.format='clrmamepro-dat'
     AND typeof(interpretation.rules_version)='text' AND interpretation.rules_version='clrmamepro-declared-text-compat-v1'
     AND typeof(publication.snapshot_key)='text'
     AND typeof(publication.catalog_key)='text' AND publication.catalog_key=snapshot.catalog_key
     AND typeof(publication.document_key)='text' AND publication.document_key=snapshot.document_key
     AND typeof(publication.interpretation_key)='text' AND publication.interpretation_key=snapshot.interpretation_key"
}

const fn identity_check() -> &'static str {
    "EXISTS (SELECT 1 FROM file_id_registries AS registry WHERE registry.registry_id=1
         AND typeof(registry.registry_id)='integer' AND typeof(registry.registry_uuid)='blob'
         AND length(registry.registry_uuid)=16)
     AND (typeof(occurrence.content_uuid)='null' OR (
         typeof(occurrence.content_uuid)='blob' AND length(occurrence.content_uuid)=16
         AND typeof(issued.content_uuid)='blob' AND length(issued.content_uuid)=16
         AND typeof(issued.registry_id)='integer' AND issued.registry_id=1))"
}

pub(super) fn roms() -> String {
    let text = optional_text(
        "rom",
        &[
            "size_text",
            "crc_text",
            "crc32_text",
            "md5_text",
            "sha1_text",
            "date",
            "serial",
            "status_text",
            "dump_status",
        ],
    );
    format!(
        "SELECT rom.occurrence_id, rom.name, rom.size_text, rom.size, rom.crc_text, rom.crc32_text,
                rom.md5_text, rom.sha1_text, rom.date, rom.serial, rom.status_text,
                rom.nodump_present, rom.baddump_present, rom.dump_status, rom.evidence_scope,
                COALESCE(form.source_order,-1) AS source_order, rom.source_line, rom.source_column,
                COALESCE((typeof(rom.occurrence_id)='integer' AND rom.occurrence_id>0
                    AND typeof(rom.claim_kind)='text' AND rom.claim_kind='cmp_rom'
                    AND typeof(rom.name)='text' AND {text}
                    AND typeof(rom.size) IN ('null','integer')
                    AND typeof(rom.nodump_present)='integer' AND rom.nodump_present IN (0,1)
                    AND typeof(rom.baddump_present)='integer' AND rom.baddump_present IN (0,1)
                    AND typeof(rom.evidence_scope)='text' AND rom.evidence_scope IN ('whole_file','whole_asset')
                    AND typeof(rom.evidence_provenance)='text' AND rom.evidence_provenance='source_declared'
                    AND typeof(rom.source_line)='integer' AND rom.source_line>0
                    AND typeof(rom.source_column)='integer' AND rom.source_column>0
                    AND typeof(form.occurrence_id)='integer' AND form.occurrence_id=rom.occurrence_id
                    AND typeof(form.source_order)='integer' AND form.source_order>=0),0) AS valid_storage
         FROM requested CROSS JOIN cmp_rom_claims AS rom
         LEFT JOIN cmp_set_rom_positions AS form ON form.occurrence_id=rom.occurrence_id
         WHERE rom.occurrence_id=requested.occurrence_id ORDER BY rom.occurrence_id"
    )
}

fn optional_text(alias: &str, fields: &[&str]) -> String {
    fields
        .iter()
        .map(|field| format!("typeof({alias}.{field}) IN ('null','text')"))
        .collect::<Vec<_>>()
        .join(" AND ")
}

pub(super) const fn samples() -> &'static str {
    "SELECT sample.occurrence_id, sample.sample_name, sample.source_field, sample.source_order,
            sample.is_quoted, sample.source_line, sample.source_column,
            COALESCE((typeof(sample.occurrence_id)='integer' AND sample.occurrence_id>0
                AND typeof(sample.claim_kind)='text' AND sample.claim_kind='cmp_sample'
                AND typeof(sample.sample_name)='text' AND typeof(sample.source_field)='text'
                AND lower(sample.source_field)='sample'
                AND typeof(sample.source_order)='integer' AND sample.source_order>=0
                AND typeof(sample.is_quoted)='integer' AND sample.is_quoted IN (0,1)
                AND typeof(sample.source_line)='integer' AND sample.source_line>0
                AND typeof(sample.source_column)='integer' AND sample.source_column>0),0) AS valid_storage
     FROM requested CROSS JOIN cmp_samples AS sample
     WHERE sample.occurrence_id=requested.occurrence_id ORDER BY sample.occurrence_id"
}

pub(super) const fn positions() -> &'static str {
    "SELECT position.occurrence_id, position.field_kind, position.source_field,
            position.source_order, position.is_quoted, position.source_line, position.source_column,
            COALESCE((typeof(position.occurrence_id)='integer' AND position.occurrence_id>0
                AND typeof(position.field_kind)='integer' AND typeof(position.source_field)='text'
                AND typeof(position.source_order)='integer' AND position.source_order>=0
                AND typeof(position.is_quoted)='integer' AND position.is_quoted IN (0,1)
                AND typeof(position.source_line)='integer' AND position.source_line>0
                AND typeof(position.source_column)='integer' AND position.source_column>0),0) AS valid_storage
     FROM requested CROSS JOIN cmp_rom_field_positions AS position
     WHERE position.occurrence_id=requested.occurrence_id
     ORDER BY position.occurrence_id, position.source_order"
}

pub(super) const fn digests() -> &'static str {
    "SELECT assertion.occurrence_id, digest.algorithm, digest.digest, assertion.scope, assertion.provenance,
            COALESCE((typeof(assertion.occurrence_id)='integer' AND assertion.occurrence_id>0
                AND typeof(assertion.digest_id)='integer' AND assertion.digest_id>0
                AND typeof(assertion.scope)='text' AND length(assertion.scope)>0
                AND typeof(assertion.provenance)='text'
                AND typeof(digest.digest_id)='integer' AND digest.digest_id>0
                AND typeof(digest.algorithm)='text' AND typeof(digest.digest)='blob'),0) AS valid_storage
     FROM requested CROSS JOIN occurrence_digest_assertions AS assertion
     LEFT JOIN digest_values AS digest ON digest.digest_id=assertion.digest_id
     WHERE assertion.occurrence_id=requested.occurrence_id
     ORDER BY assertion.occurrence_id, assertion.scope, assertion.digest_id, assertion.provenance"
}

pub(super) const fn merges() -> &'static str {
    "SELECT declaration.occurrence_id, declaration.relationship_id, declaration.merge_name,
            COALESCE((typeof(declaration.occurrence_id)='integer' AND declaration.occurrence_id>0
                AND typeof(declaration.relationship_id)='integer' AND declaration.relationship_id>0
                AND typeof(declaration.merge_name)='text' AND typeof(declaration.source_reference_kind)='text'
                AND declaration.source_reference_kind='clrmamepro_rom_merge'),0) AS valid_storage,
            COALESCE((typeof(occurrence.occurrence_id)='integer' AND occurrence.claim_kind='cmp_rom'
                AND sets.source_element_kind='cmp_set' AND typeof(groups.snapshot_key)='text'
                AND typeof(identity.relationship_id)='integer' AND identity.relationship_id=declaration.relationship_id
                AND typeof(identity.origin)='text' AND identity.origin='source'
                AND typeof(identity.snapshot_key)='text' AND identity.snapshot_key=groups.snapshot_key
                AND typeof(reported.relationship_id)='integer' AND reported.relationship_id=declaration.relationship_id
                AND typeof(reported.source_reference_kind)='text'
                AND reported.source_reference_kind='clrmamepro_rom_merge'),0) AS valid_owner
     FROM requested CROSS JOIN clrmamepro_rom_merges AS declaration
     LEFT JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=declaration.occurrence_id
     LEFT JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
     LEFT JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id
     LEFT JOIN catalog_relationships AS identity ON identity.relationship_id=declaration.relationship_id
     LEFT JOIN reported_catalog_relationships AS reported ON reported.relationship_id=declaration.relationship_id
     WHERE declaration.occurrence_id=requested.occurrence_id ORDER BY declaration.occurrence_id"
}
