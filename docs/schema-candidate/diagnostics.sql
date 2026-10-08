-- Exact diagnostic evidence is separate from catalog values and external originals.
CREATE TABLE catalog_import_messages (
    message_id INTEGER PRIMARY KEY NOT NULL CHECK (message_id > 0),
    import_id INTEGER NOT NULL REFERENCES catalog_imports(import_id),
    message_order INTEGER NOT NULL CHECK (message_order >= 0),
    severity TEXT NOT NULL CHECK (severity IN ('warning', 'error')),
    code TEXT NOT NULL,
    message TEXT NOT NULL,
    record_kind TEXT,
    record_name TEXT,
    field_name TEXT,
    offending_text TEXT,
    edition_id INTEGER REFERENCES catalog_editions(edition_id),
    source_file_id INTEGER NOT NULL REFERENCES catalog_source_files(source_file_id),
    reading_rules_id INTEGER NOT NULL REFERENCES catalog_reading_rules(reading_rules_id),
    excerpt BLOB,
    source_view TEXT CHECK (source_view IN ('retained_original_bytes', 'transport_decoded_xml_bytes')),
    excerpt_source_start INTEGER CHECK (excerpt_source_start >= 0),
    excerpt_problem_start INTEGER,
    excerpt_problem_end INTEGER,
    source_problem_start INTEGER,
    source_problem_end INTEGER,
    original_problem_start INTEGER,
    original_problem_end INTEGER,
    line INTEGER CHECK (line > 0),
    column INTEGER CHECK (column > 0),
    location_view TEXT CHECK (location_view IN ('retained_original_text', 'transport_decoded_xml_text', 'decoded_dat_text')),
    column_convention TEXT CHECK (column_convention = 'one_based_unicode_scalar'),
    FOREIGN KEY (import_id, source_file_id, reading_rules_id)
        REFERENCES catalog_imports(import_id, source_file_id, reading_rules_id),
    UNIQUE (import_id, message_order),
    UNIQUE (message_id, edition_id),
    CHECK ((excerpt IS NULL AND excerpt_source_start IS NULL) OR
        (excerpt IS NOT NULL AND excerpt_source_start IS NOT NULL AND source_view IS NOT NULL)),
    CHECK ((excerpt_problem_start IS NULL AND excerpt_problem_end IS NULL) OR
        (excerpt_problem_start IS NOT NULL AND excerpt_problem_end IS NOT NULL AND excerpt IS NOT NULL
         AND 0 <= excerpt_problem_start AND excerpt_problem_start <= excerpt_problem_end AND excerpt_problem_end <= length(excerpt))),
    CHECK ((source_problem_start IS NULL AND source_problem_end IS NULL) OR
        (source_problem_start IS NOT NULL AND source_problem_end IS NOT NULL AND source_view IS NOT NULL
         AND 0 <= source_problem_start AND source_problem_start <= source_problem_end)),
    CHECK ((original_problem_start IS NULL AND original_problem_end IS NULL) OR
        (original_problem_start IS NOT NULL AND original_problem_end IS NOT NULL
         AND 0 <= original_problem_start AND original_problem_start <= original_problem_end)),
    CHECK ((location_view IS NULL AND column_convention IS NULL) OR
        (location_view IS NOT NULL AND column_convention IS NOT NULL))
) STRICT;

CREATE TABLE catalog_import_message_elements (
    message_id INTEGER NOT NULL,
    source_element_id INTEGER NOT NULL,
    edition_id INTEGER NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('primary', 'related')),
    PRIMARY KEY (message_id, source_element_id, role),
    FOREIGN KEY (message_id, edition_id) REFERENCES catalog_import_messages(message_id, edition_id),
    FOREIGN KEY (source_element_id, edition_id) REFERENCES catalog_source_elements(source_element_id, edition_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX catalog_import_message_elements_by_owner
    ON catalog_import_message_elements(source_element_id,message_id);
CREATE INDEX catalog_import_message_elements_by_edition
    ON catalog_import_message_elements(edition_id,message_id);

CREATE TABLE catalog_import_message_external_evidence (
    message_id INTEGER NOT NULL REFERENCES catalog_import_messages(message_id),
    compared_source_element_id INTEGER NOT NULL,
    target_edition_id INTEGER NOT NULL REFERENCES published_catalog_editions(edition_id),
    evidence_role TEXT NOT NULL CHECK (evidence_role IN ('conflicting_candidate', 'comparison_reference')),
    PRIMARY KEY (message_id, compared_source_element_id, evidence_role),
    FOREIGN KEY (compared_source_element_id, target_edition_id)
        REFERENCES catalog_source_elements(source_element_id, edition_id)
) STRICT, WITHOUT ROWID;

CREATE VIEW candidate_message_integrity_problems AS
SELECT 'message_import_edition' AS problem, message.message_id AS owner_id, coalesce(message.edition_id,run.edition_id) AS edition_id
FROM catalog_import_messages AS message JOIN catalog_imports AS run USING (import_id)
WHERE message.edition_id IS NOT run.edition_id
UNION ALL
SELECT 'message_original_bounds', message.message_id, message.edition_id
FROM catalog_import_messages AS message JOIN catalog_source_files AS source USING (source_file_id)
WHERE message.original_problem_end > source.byte_length
    OR (message.source_view = 'retained_original_bytes' AND
        (message.source_problem_end > source.byte_length OR length(message.excerpt) > source.byte_length - message.excerpt_source_start))
UNION ALL
SELECT 'message_decoded_bounds',message.message_id,message.edition_id
FROM catalog_import_messages AS message LEFT JOIN catalog_decoded_xml_views AS view USING(source_file_id)
WHERE message.source_view='transport_decoded_xml_bytes'
AND (message.source_problem_end IS NOT NULL OR message.excerpt_source_start IS NOT NULL)
AND (view.source_file_id IS NULL OR message.source_problem_end > view.byte_length OR length(message.excerpt) > view.byte_length-message.excerpt_source_start)
UNION ALL
SELECT 'message_highlight_mapping',message.message_id,message.edition_id
FROM catalog_import_messages AS message
WHERE message.source_problem_start IS NOT NULL AND message.excerpt_source_start IS NOT NULL
AND (
    (message.source_problem_start >= message.excerpt_source_start
     AND message.source_problem_end - message.excerpt_source_start <= length(message.excerpt)
     AND (message.excerpt_problem_start IS NOT message.source_problem_start-message.excerpt_source_start
          OR message.excerpt_problem_end IS NOT message.source_problem_end-message.excerpt_source_start))
    OR
    ((message.source_problem_start < message.excerpt_source_start
      OR message.source_problem_end-message.excerpt_source_start > length(message.excerpt))
     AND (message.excerpt_problem_start IS NOT NULL OR message.excerpt_problem_end IS NOT NULL))
)
UNION ALL
SELECT 'message_coordinate_views',message.message_id,message.edition_id
FROM catalog_import_messages AS message
WHERE message.source_view='retained_original_bytes'
AND message.source_problem_start IS NOT NULL AND message.original_problem_start IS NOT NULL
AND (message.source_problem_start IS NOT message.original_problem_start
     OR message.source_problem_end IS NOT message.original_problem_end)
UNION ALL
SELECT 'external_evidence_is_incoming', evidence.message_id, message.edition_id
FROM catalog_import_message_external_evidence AS evidence JOIN catalog_import_messages AS message USING (message_id)
WHERE evidence.target_edition_id IS message.edition_id;

CREATE TRIGGER candidate_message_coordinates_insert BEFORE INSERT ON catalog_import_messages
WHEN (
    NEW.source_problem_start IS NOT NULL AND NEW.excerpt_source_start IS NOT NULL AND (
        (NEW.source_problem_start >= NEW.excerpt_source_start
         AND NEW.source_problem_end-NEW.excerpt_source_start <= length(NEW.excerpt)
         AND (NEW.excerpt_problem_start IS NOT NEW.source_problem_start-NEW.excerpt_source_start
              OR NEW.excerpt_problem_end IS NOT NEW.source_problem_end-NEW.excerpt_source_start))
        OR
        ((NEW.source_problem_start < NEW.excerpt_source_start
          OR NEW.source_problem_end-NEW.excerpt_source_start > length(NEW.excerpt))
         AND (NEW.excerpt_problem_start IS NOT NULL OR NEW.excerpt_problem_end IS NOT NULL))
    )
) OR (
    NEW.source_view='retained_original_bytes'
    AND NEW.source_problem_start IS NOT NULL AND NEW.original_problem_start IS NOT NULL
    AND (NEW.source_problem_start IS NOT NEW.original_problem_start
         OR NEW.source_problem_end IS NOT NEW.original_problem_end)
)
BEGIN SELECT RAISE(ABORT,'diagnostic coordinates do not match their source views'); END;

CREATE TRIGGER candidate_message_coordinates_update BEFORE UPDATE ON catalog_import_messages
WHEN (
    NEW.source_problem_start IS NOT NULL AND NEW.excerpt_source_start IS NOT NULL AND (
        (NEW.source_problem_start >= NEW.excerpt_source_start
         AND NEW.source_problem_end-NEW.excerpt_source_start <= length(NEW.excerpt)
         AND (NEW.excerpt_problem_start IS NOT NEW.source_problem_start-NEW.excerpt_source_start
              OR NEW.excerpt_problem_end IS NOT NEW.source_problem_end-NEW.excerpt_source_start))
        OR
        ((NEW.source_problem_start < NEW.excerpt_source_start
          OR NEW.source_problem_end-NEW.excerpt_source_start > length(NEW.excerpt))
         AND (NEW.excerpt_problem_start IS NOT NULL OR NEW.excerpt_problem_end IS NOT NULL))
    )
) OR (
    NEW.source_view='retained_original_bytes'
    AND NEW.source_problem_start IS NOT NULL AND NEW.original_problem_start IS NOT NULL
    AND (NEW.source_problem_start IS NOT NEW.original_problem_start
         OR NEW.source_problem_end IS NOT NEW.original_problem_end)
)
BEGIN SELECT RAISE(ABORT,'diagnostic coordinates do not match their source views'); END;

CREATE TRIGGER candidate_message_ancestry_insert BEFORE INSERT ON catalog_import_messages
WHEN NOT EXISTS (SELECT 1 FROM catalog_imports WHERE import_id=NEW.import_id AND edition_id IS NEW.edition_id)
BEGIN SELECT RAISE(ABORT,'message must use the actual import edition'); END;

CREATE TRIGGER candidate_import_message_reverse BEFORE UPDATE ON catalog_imports
WHEN EXISTS(SELECT 1 FROM catalog_import_messages AS message WHERE message.import_id=OLD.import_id AND
    (message.edition_id IS NOT NEW.edition_id OR message.source_file_id<>NEW.source_file_id OR message.reading_rules_id<>NEW.reading_rules_id))
BEGIN SELECT RAISE(ABORT,'import rewrite would invalidate existing messages'); END;
CREATE TRIGGER candidate_message_ancestry_update BEFORE UPDATE ON catalog_import_messages
WHEN NOT EXISTS (SELECT 1 FROM catalog_imports WHERE import_id=NEW.import_id AND edition_id IS NEW.edition_id)
BEGIN SELECT RAISE(ABORT,'message must use the actual import edition'); END;
