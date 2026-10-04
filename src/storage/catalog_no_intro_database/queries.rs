pub(super) const SNAPSHOT: &str = "
SELECT snapshots.snapshot_key, sources.source_key, sources.display_name AS source_name,
       catalogs.catalog_key, catalogs.display_name AS catalog_name, snapshots.document_key,
       snapshots.interpretation_key, interpretations.format, versions.declared_version,
       publication.snapshot_key IS NOT NULL AS published
FROM catalog_snapshots AS snapshots
JOIN catalogs ON catalogs.catalog_key = snapshots.catalog_key
JOIN publishing_sources AS sources ON sources.source_key = catalogs.source_key
JOIN documents ON documents.document_key = snapshots.document_key
JOIN parser_interpretations AS interpretations
  ON interpretations.interpretation_key = snapshots.interpretation_key
LEFT JOIN snapshot_publications AS publication
  ON publication.snapshot_key = snapshots.snapshot_key
 AND publication.catalog_key = snapshots.catalog_key
 AND publication.document_key = snapshots.document_key
 AND publication.interpretation_key = snapshots.interpretation_key
LEFT JOIN catalog_snapshot_versions AS versions ON versions.snapshot_key = snapshots.snapshot_key
WHERE snapshots.snapshot_key = ?";

pub(super) const GROUPS: &str = "
SELECT set_group_id, kind, list_order,
       typeof(set_group_id)='integer' AND typeof(list_order)='integer' AS valid
FROM catalog_set_groups
WHERE snapshot_key = ? ORDER BY kind, list_order LIMIT 2";

pub(super) fn games(after_cursor: bool) -> String {
    let continuation = if after_cursor {
        " AND (sets.list_order, sets.set_id) > (?, ?)"
    } else {
        ""
    };
    format!(
        "SELECT sets.set_id, sets.set_name, sets.list_order, sets.source_element_kind, \
                sets.source_line AS line, sets.source_column AS column, \
                native.set_id AS native_id, \
                native.name_source_order, native.name_source_line, native.name_source_column, \
                typeof(sets.set_id)='integer' AND typeof(sets.set_group_id)='integer' \
                AND typeof(sets.list_order)='integer' AND typeof(sets.source_line)='integer' \
                AND typeof(sets.source_column)='integer' \
                AND typeof(native.set_id)='integer' \
                AND typeof(native.name_source_order)='integer' \
                AND typeof(native.name_source_line)='integer' \
                AND typeof(native.name_source_column)='integer' AS valid \
         FROM catalog_sets AS sets LEFT JOIN no_intro_database_games AS native \
           ON native.set_id = sets.set_id \
         WHERE sets.set_group_id = ?{continuation} \
         ORDER BY sets.list_order, sets.set_id LIMIT ?"
    )
}

pub(super) const fn cursor_anchor() -> &'static str {
    "SELECT sets.set_id, sets.set_name, sets.list_order, sets.source_element_kind, \
            sets.source_line AS line, sets.source_column AS column, \
            native.set_id AS native_id, native.name_source_order, \
            native.name_source_line, native.name_source_column, \
            typeof(sets.set_id)='integer' AND typeof(sets.set_group_id)='integer' \
            AND typeof(sets.list_order)='integer' AND typeof(sets.source_line)='integer' \
            AND typeof(sets.source_column)='integer' \
            AND typeof(native.set_id)='integer' \
            AND typeof(native.name_source_order)='integer' \
            AND typeof(native.name_source_line)='integer' \
            AND typeof(native.name_source_column)='integer' AS valid \
     FROM catalog_sets AS sets LEFT JOIN no_intro_database_games AS native \
       ON native.set_id=sets.set_id \
     WHERE sets.set_group_id=? AND sets.set_id=? AND sets.list_order=?"
}

/// Drive owner lookups from a bounded request set so unrelated snapshots are never scanned.
pub(super) fn owners(select: &str, table: &str, owner: &str, count: usize, order: &str) -> String {
    let values = std::iter::repeat_n("(?)", count)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "WITH requested(owner_id) AS (VALUES {values}) \
         SELECT native.{owner} AS owner_id, {select} \
         FROM requested CROSS JOIN {table} AS native \
         WHERE native.{owner} = requested.owner_id \
         ORDER BY native.{owner}, {order}"
    )
}

fn requested_values(count: usize) -> String {
    std::iter::repeat_n("(?)", count)
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn archive_rows(count: usize) -> String {
    let values = requested_values(count);
    format!(
        "WITH requested(owner_id) AS (VALUES {values}) \
         SELECT native.archive_id, native.set_id, native.source_order, \
                native.source_line AS line, native.source_column AS column, \
                native.additional, native.adult, native.aftermarket, native.alt, native.bios, \
                native.categories, native.complete, native.dat, native.datter_note, \
                native.description, native.devstatus, native.gameid1, native.gameid2, \
                native.langchecked, native.languages, native.licensed, native.listed, \
                native.mergename, native.name, native.name_alt, native.number, native.physical, \
                native.region, native.regparent, native.showlang, native.special1, native.special2, \
                native.sticky_note, native.version1, native.version2, marker.marker AS clone_marker, \
                clone_link.declared_target_number AS clone_target, \
                clone_link.relationship_id AS clone_relationship_id, \
                merge_link.declared_mergeof AS mergeof, \
                merge_link.relationship_id AS merge_relationship_id, \
                typeof(native.archive_id)='integer' AND typeof(native.set_id)='integer' \
                AND typeof(native.source_order)='integer' AND typeof(native.source_line)='integer' \
                AND typeof(native.source_column)='integer' \
                AND (clone_link.relationship_id IS NULL OR typeof(clone_link.relationship_id)='integer') \
                AND (merge_link.relationship_id IS NULL OR typeof(merge_link.relationship_id)='integer') AS valid, \
                COALESCE(typeof(game.set_id)='integer' AND game.set_id=native.set_id \
                  AND typeof(sets.set_id)='integer' AND typeof(sets.set_group_id)='integer' \
                  AND sets.source_element_kind='no_intro_database_game' \
                  AND typeof(groups.set_group_id)='integer' AND groups.kind='root' \
                  AND export.snapshot_key=groups.snapshot_key \
                  AND snapshot.snapshot_key=groups.snapshot_key \
                  AND interpretation.format IN ('no-intro-database-xml-compatible', \
                                                 'no-intro-database-xml-nul-compatible') \
                  AND (clone_link.archive_id IS NULL OR ( \
                    typeof(clone_link.relationship_id)='integer' AND clone_link.relationship_id>0 \
                    AND typeof(clone_reported.relationship_id)='integer' \
                    AND clone_reported.source_reference_kind='no_intro_database_archive_clone' \
                    AND typeof(clone_registry.relationship_id)='integer' \
                    AND clone_registry.origin='source' \
                    AND clone_registry.snapshot_key=groups.snapshot_key)) \
                  AND (merge_link.archive_id IS NULL OR ( \
                    typeof(merge_link.relationship_id)='integer' AND merge_link.relationship_id>0 \
                    AND typeof(merge_reported.relationship_id)='integer' \
                    AND merge_reported.source_reference_kind='no_intro_database_archive_mergeof' \
                    AND typeof(merge_registry.relationship_id)='integer' \
                    AND merge_registry.origin='source' \
                    AND merge_registry.snapshot_key=groups.snapshot_key)), 0) AS registry_valid \
         FROM requested CROSS JOIN no_intro_archive_descriptions AS native \
         LEFT JOIN no_intro_archive_clone_markers AS marker USING(archive_id) \
         LEFT JOIN no_intro_archive_clone_links AS clone_link USING(archive_id) \
         LEFT JOIN no_intro_archive_merge_links AS merge_link USING(archive_id) \
         LEFT JOIN reported_catalog_relationships AS clone_reported \
           ON clone_reported.relationship_id=clone_link.relationship_id \
         LEFT JOIN catalog_relationships AS clone_registry \
           ON clone_registry.relationship_id=clone_link.relationship_id \
         LEFT JOIN reported_catalog_relationships AS merge_reported \
           ON merge_reported.relationship_id=merge_link.relationship_id \
         LEFT JOIN catalog_relationships AS merge_registry \
           ON merge_registry.relationship_id=merge_link.relationship_id \
         LEFT JOIN no_intro_database_games AS game ON game.set_id=native.set_id \
         LEFT JOIN catalog_sets AS sets ON sets.set_id=native.set_id \
         LEFT JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id \
         LEFT JOIN no_intro_exports AS export ON export.snapshot_key=groups.snapshot_key \
         LEFT JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key \
         LEFT JOIN parser_interpretations AS interpretation \
           ON interpretation.interpretation_key=snapshot.interpretation_key \
         WHERE native.archive_id=requested.owner_id ORDER BY native.archive_id"
    )
}

pub(super) fn child_rows(count: usize) -> String {
    let values = requested_values(count);
    let branches = [
        ("no_intro_archive_descriptions", "archive_id", "archive"),
        ("no_intro_dump_sources", "dump_source_id", "dump_source"),
        ("no_intro_releases", "release_id", "release"),
    ]
    .map(|(table, owner, kind)| {
        format!(
            "SELECT native.set_id AS game_id, '{kind}' AS child_kind, native.{owner} AS child_id, \
                native.source_order, native.source_line AS line, native.source_column AS column, \
                typeof(native.set_id)='integer' AND typeof(native.{owner})='integer' \
                AND typeof(native.source_order)='integer' AND typeof(native.source_line)='integer' \
                AND typeof(native.source_column)='integer' AS valid \
         FROM requested CROSS JOIN {table} AS native WHERE native.set_id=requested.game_id"
        )
    })
    .join(" UNION ALL ");
    format!(
        "WITH requested(game_id) AS (VALUES {values}) {branches} \
         ORDER BY game_id, source_order, child_id"
    )
}

pub(super) fn nfo_hashes(count: usize) -> String {
    let values = requested_values(count);
    format!(
        "WITH requested(owner_id) AS (VALUES {values}) \
         SELECT native.release_id AS owner_id, native.source_hash_field, native.hash_id, \
                digests.algorithm, digests.digest, native.presence, native.scope, native.invalid_literal, \
                COALESCE(typeof(native.release_id)='integer' AND native.release_id>0 \
                  AND typeof(native.source_hash_field)='text' \
                  AND native.source_hash_field IN ('nfocrc','nfo_crc32') \
                  AND typeof(native.presence)='text' AND native.presence='present' \
                  AND typeof(native.scope)='text' AND native.scope='nfo_companion' AND ( \
                    (typeof(native.hash_id)='integer' AND native.hash_id>0 \
                     AND typeof(digests.digest_id)='integer' \
                     AND typeof(digests.algorithm)='text' AND digests.algorithm='crc32' \
                     AND typeof(digests.digest)='blob' AND length(digests.digest)=4 \
                     AND native.invalid_literal IS NULL) OR \
                    (native.hash_id IS NULL AND digests.digest_id IS NULL \
                     AND typeof(native.invalid_literal)='text' \
                     AND (length(native.invalid_literal)<>8 \
                          OR native.invalid_literal GLOB '*[^0-9A-Fa-f]*'))), 0) AS valid \
         FROM requested CROSS JOIN no_intro_release_nfo_hashes AS native \
         LEFT JOIN digest_values AS digests ON digests.digest_id=native.hash_id \
         WHERE native.release_id=requested.owner_id ORDER BY native.release_id, native.source_hash_field"
    )
}

/// Check both directions without depending on any surviving history in the game.
pub(super) fn selected_file_owners(count: usize) -> String {
    let values = requested_values(count);
    let reverse = [
        (
            "no_intro_dump_sources",
            "no_intro_dump_files",
            "dump_source_id",
            "source",
        ),
        (
            "no_intro_releases",
            "no_intro_release_files",
            "release_id",
            "release",
        ),
    ]
    .map(|(owners, files, owner_id, kind)| {
        format!(
            "SELECT requested.game_id, file.occurrence_id, \
                COALESCE(typeof(owner.{owner_id})='integer' AND typeof(owner.set_id)='integer' \
                  AND owner.{owner_id}>0 AND owner.set_id=requested.game_id \
                  AND typeof(file.{owner_id})='integer' AND typeof(file.set_id)='integer' \
                  AND typeof(file.occurrence_id)='integer' AND file.occurrence_id>0 \
                  AND file.set_id=requested.game_id \
                  AND file.claim_kind='no_intro_database_{kind}_file' \
                  AND typeof(occurrence.occurrence_id)='integer' \
                  AND typeof(occurrence.record_id)='integer' \
                  AND typeof(occurrence.occurrence_order)='integer' \
                  AND occurrence.record_id=requested.game_id \
                  AND occurrence.claim_kind='no_intro_database_{kind}_file', 0) AS valid \
         FROM requested CROSS JOIN {files} AS file \
         LEFT JOIN {owners} AS owner ON owner.{owner_id}=file.{owner_id} \
         LEFT JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=file.occurrence_id \
         WHERE file.set_id=requested.game_id"
        )
    })
    .join(" UNION ALL ");
    format!(
        "WITH requested(game_id) AS (VALUES {values}) \
         SELECT requested.game_id, occurrence.occurrence_id, \
                COALESCE(typeof(occurrence.occurrence_id)='integer' AND occurrence.occurrence_id>0 \
                  AND typeof(occurrence.record_id)='integer' \
                  AND typeof(occurrence.occurrence_order)='integer' AND ( \
                    (occurrence.claim_kind='no_intro_database_source_file' \
                     AND typeof(source_file.occurrence_id)='integer' \
                     AND release_file.occurrence_id IS NULL \
                     AND typeof(source_file.dump_source_id)='integer' \
                     AND typeof(source_file.set_id)='integer' \
                     AND source_file.set_id=requested.game_id \
                     AND source_file.claim_kind='no_intro_database_source_file' \
                     AND NOT EXISTS(SELECT 1 FROM no_intro_release_file_field_positions AS release_positions \
                                    WHERE release_positions.occurrence_id=occurrence.occurrence_id) \
                     AND NOT EXISTS(SELECT 1 FROM no_intro_release_file_digests AS release_digests \
                                    WHERE release_digests.occurrence_id=occurrence.occurrence_id) \
                     AND typeof(source_owner.dump_source_id)='integer' \
                     AND typeof(source_owner.set_id)='integer' \
                     AND source_owner.set_id=requested.game_id) \
                    OR (occurrence.claim_kind='no_intro_database_release_file' \
                     AND typeof(release_file.occurrence_id)='integer' \
                     AND source_file.occurrence_id IS NULL \
                     AND typeof(release_file.release_id)='integer' \
                     AND typeof(release_file.set_id)='integer' \
                     AND release_file.set_id=requested.game_id \
                     AND release_file.claim_kind='no_intro_database_release_file' \
                     AND NOT EXISTS(SELECT 1 FROM no_intro_dump_file_field_positions AS source_positions \
                                    WHERE source_positions.occurrence_id=occurrence.occurrence_id) \
                     AND NOT EXISTS(SELECT 1 FROM no_intro_dump_file_digests AS source_digests \
                                    WHERE source_digests.occurrence_id=occurrence.occurrence_id) \
                     AND typeof(release_owner.release_id)='integer' \
                     AND typeof(release_owner.set_id)='integer' \
                     AND release_owner.set_id=requested.game_id)), 0) AS valid \
         FROM requested CROSS JOIN asset_occurrences AS occurrence \
         LEFT JOIN no_intro_dump_files AS source_file ON source_file.occurrence_id=occurrence.occurrence_id \
         LEFT JOIN no_intro_release_files AS release_file ON release_file.occurrence_id=occurrence.occurrence_id \
         LEFT JOIN no_intro_dump_sources AS source_owner ON source_owner.dump_source_id=source_file.dump_source_id \
         LEFT JOIN no_intro_releases AS release_owner ON release_owner.release_id=release_file.release_id \
         WHERE occurrence.record_id=requested.game_id UNION ALL {reverse}"
    )
}

pub(super) fn position_rows(table: &str, owner: &str, count: usize) -> String {
    owners(
        &format!(
            "native.field_kind, native.source_order, native.source_line AS line, \
         native.source_column AS column, \
         typeof(native.{owner})='integer' AND typeof(native.field_kind)='integer' \
         AND typeof(native.source_order)='integer' AND typeof(native.source_line)='integer' \
         AND typeof(native.source_column)='integer' AS valid"
        ),
        table,
        owner,
        count,
        "native.source_order",
    )
}

#[derive(Clone, Copy)]
pub(super) enum FileOwner {
    DumpSource,
    Release,
}

pub(super) struct FileShape {
    pub table: &'static str,
    pub digest_table: &'static str,
    pub position_table: &'static str,
    pub owner_column: &'static str,
    pub owner_table: &'static str,
    pub claim_kind: &'static str,
    pub fields: &'static [(&'static str, Option<i64>)],
}

impl FileOwner {
    pub(super) const fn shape(self) -> FileShape {
        match self {
            Self::DumpSource => FileShape {
                table: "no_intro_dump_files",
                digest_table: "no_intro_dump_file_digests",
                position_table: "no_intro_dump_file_field_positions",
                owner_column: "dump_source_id",
                owner_table: "no_intro_dump_sources",
                claim_kind: "no_intro_database_source_file",
                fields: &[
                    ("bad", None),
                    ("", Some(0)),
                    ("date", None),
                    ("extension", None),
                    ("filter", None),
                    ("forcename", None),
                    ("forcescenename", None),
                    ("format", None),
                    ("header", None),
                    ("id", None),
                    ("item", None),
                    ("", Some(1)),
                    ("mia", None),
                    ("note", None),
                    ("", Some(4)),
                    ("origin_size", None),
                    ("serial", None),
                    ("", Some(2)),
                    ("", Some(3)),
                    ("source_size", None),
                    ("unique", None),
                    ("update_type", None),
                    ("version", None),
                ],
            },
            Self::Release => FileShape {
                table: "no_intro_release_files",
                digest_table: "no_intro_release_file_digests",
                position_table: "no_intro_release_file_field_positions",
                owner_column: "release_id",
                owner_table: "no_intro_releases",
                claim_kind: "no_intro_database_release_file",
                fields: &[
                    ("bad", None),
                    ("", Some(0)),
                    ("extension", None),
                    ("forcename", None),
                    ("forcescenename", None),
                    ("format", None),
                    ("header", None),
                    ("id", None),
                    ("item", None),
                    ("", Some(1)),
                    ("note", None),
                    ("serial", None),
                    ("", Some(2)),
                    ("", Some(3)),
                    ("source_size", None),
                    ("update_type", None),
                    ("version", None),
                ],
            },
        }
    }
}

pub(super) fn file_rows(owner: FileOwner, count: usize) -> String {
    let FileShape {
        table,
        digest_table,
        owner_column,
        owner_table,
        claim_kind,
        fields,
        ..
    } = owner.shape();
    let values = std::iter::repeat_n("(?)", count)
        .collect::<Vec<_>>()
        .join(",");
    let presence = fields.iter().map(|(column, digest_kind)| {
        digest_kind.as_ref().map_or_else(|| format!("native.\"{column}\" IS NOT NULL"), |kind| format!("EXISTS(SELECT 1 FROM {digest_table} d WHERE d.occurrence_id=native.occurrence_id AND d.field_kind={kind})"))
    }).map(|value| format!("CAST(({value}) AS TEXT)")).collect::<Vec<_>>().join(" || ");
    let digest_kinds = fields
        .iter()
        .filter_map(|(_, kind)| *kind)
        .map(|kind| kind.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let algorithm = "CASE d.field_kind WHEN 0 THEN 'crc32' WHEN 1 THEN 'md5' WHEN 2 THEN 'sha1' ELSE 'sha256' END";
    let bytes = "CASE d.field_kind WHEN 0 THEN 4 WHEN 1 THEN 16 WHEN 2 THEN 20 ELSE 32 END";
    let invalid_digest = format!(
        "EXISTS(SELECT 1 FROM {digest_table} d LEFT JOIN digest_values v ON v.digest_id=d.digest_id \
         WHERE d.occurrence_id=native.occurrence_id AND \
         (typeof(d.occurrence_id)='integer' AND typeof(d.field_kind)='integer' \
          AND d.field_kind IN ({digest_kinds}) AND \
          ((typeof(d.digest_id)='integer' AND d.digest_id>0 AND v.algorithm=({algorithm}) \
            AND typeof(v.digest)='blob' AND length(v.digest)=({bytes}) AND d.invalid_literal IS NULL) OR \
           (d.digest_id IS NULL AND typeof(d.invalid_literal)='text' AND \
            (length(d.invalid_literal)<>2*({bytes}) OR d.invalid_literal GLOB '*[^0-9A-Fa-f]*'))) \
         ) IS NOT TRUE)"
    );
    format!(
        "WITH requested(owner_id) AS (VALUES {values}) \
         SELECT native.{owner_column} AS owner_id, owner.set_id AS expected_set_id, \
                native.occurrence_id, native.set_id AS actual_set_id, \
                occurrence.record_id AS occurrence_set_id, occurrence.occurrence_order, native.source_order, \
                native.source_line AS line, native.source_column AS column, {presence} AS presence, \
                typeof(owner.{owner_column})='integer' AND typeof(owner.set_id)='integer' \
                AND typeof(native.{owner_column})='integer' AND typeof(native.occurrence_id)='integer' \
                AND typeof(native.set_id)='integer' AND typeof(native.source_order)='integer' \
                AND typeof(native.source_line)='integer' AND typeof(native.source_column)='integer' \
                AND typeof(occurrence.occurrence_id)='integer' AND typeof(occurrence.record_id)='integer' \
                AND typeof(occurrence.occurrence_order)='integer' AND native.claim_kind='{claim_kind}' \
                AND occurrence.claim_kind='{claim_kind}' \
                AND native.evidence_provenance='source_declared' \
                AND native.evidence_scope IN ('unknown','whole_file') \
                AND NOT ({invalid_digest}) AS valid \
         FROM requested CROSS JOIN {owner_table} AS owner \
         CROSS JOIN {table} AS native \
         LEFT JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=native.occurrence_id \
         WHERE owner.{owner_column}=requested.owner_id \
           AND native.{owner_column}=owner.{owner_column} \
         ORDER BY native.{owner_column}, native.source_order"
    )
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use diesel::{
        Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
        sql_types::{BigInt, Text},
    };

    use crate::database::Database;

    use super::{FileOwner, cursor_anchor, file_rows, games, owners, position_rows};

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    #[derive(QueryableByName)]
    struct PlanRow {
        #[diesel(sql_type = Text)]
        detail: String,
    }

    #[derive(QueryableByName)]
    struct ValueRow {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        value: i64,
    }

    #[derive(QueryableByName)]
    struct TriggerRow {
        #[diesel(sql_type = Text)]
        name: String,
    }

    fn explain(
        connection: &mut SqliteConnection,
        sql: &str,
        parameters: usize,
    ) -> TestResult<Vec<String>> {
        let mut query =
            sql_query(format!("EXPLAIN QUERY PLAN {sql}")).into_boxed::<diesel::sqlite::Sqlite>();
        for _ in 0..parameters {
            query = query.bind::<diesel::sql_types::BigInt, _>(1);
        }
        Ok(query
            .load::<PlanRow>(connection)?
            .into_iter()
            .map(|row| row.detail)
            .collect())
    }

    fn assert_indexed_seek(
        connection: &mut SqliteConnection,
        sql: &str,
        parameters: usize,
        alias: &str,
    ) -> TestResult {
        let plan = explain(connection, sql, parameters)?;
        assert!(
            plan.iter()
                .any(|step| step.starts_with(&format!("SEARCH {alias} "))),
            "query did not seek the {alias} owner index: {plan:?}"
        );
        assert!(
            !plan
                .iter()
                .any(|step| step.starts_with(&format!("SCAN {alias}"))),
            "query scans unrelated {alias} owners: {plan:?}"
        );
        Ok(())
    }

    fn populate_native_owners(connection: &mut SqliteConnection) -> TestResult {
        sql_query("PRAGMA foreign_keys=OFF").execute(connection)?;
        connection.transaction(|connection| {
            // This fixture measures lookup plans on the canonical tables and indexes, not the
            // publication guards that normally require a fully staged snapshot before insertion.
            let triggers = sql_query("SELECT name FROM sqlite_schema WHERE type='trigger'")
                .load::<TriggerRow>(connection)?;
            for trigger in triggers {
                let quoted_name = trigger.name.replace('"', "\"\"");
                sql_query(format!("DROP TRIGGER \"{quoted_name}\"")).execute(connection)?;
            }
            // Populate unrelated snapshot groups, games, every native owner family, every
            // attribute-position family, and source/release asset occurrences.
            for set_id in 1_i64..=512 {
                sql_query("INSERT INTO catalog_set_groups(set_group_id,snapshot_key,kind,list_order) VALUES(?,?,'root',0)")
                .bind::<BigInt, _>(set_id)
                .bind::<Text, _>(format!("unrelated-snapshot-{set_id}"))
                .execute(connection)?;
                sql_query("INSERT INTO catalog_sets(set_id,set_group_id,source_element_kind,list_order,set_name,source_line,source_column) VALUES(?,?,'no_intro_database_game',?,'unrelated',1,1)")
                .bind::<BigInt, _>(set_id)
                .bind::<BigInt, _>(set_id)
                .bind::<BigInt, _>(set_id)
                .execute(connection)?;
                sql_query("INSERT INTO no_intro_database_games(set_id,name_source_order,name_source_line,name_source_column) VALUES(?,0,1,1)")
                .bind::<BigInt, _>(set_id)
                .execute(connection)?;
                sql_query("INSERT INTO no_intro_archive_descriptions(archive_id,set_id,source_order,source_line,source_column) VALUES(?,?,0,1,1)")
                .bind::<BigInt, _>(set_id)
                .bind::<BigInt, _>(set_id)
                .execute(connection)?;
                sql_query("INSERT INTO no_intro_archive_field_positions(archive_id,field_kind,source_order,source_line,source_column) VALUES(?,0,1,1,1)")
                .bind::<BigInt, _>(set_id)
                .execute(connection)?;
                sql_query("INSERT INTO no_intro_dump_sources(dump_source_id,set_id,source_order,source_line,source_column) VALUES(?,?,0,1,1)")
                .bind::<BigInt, _>(set_id)
                .bind::<BigInt, _>(set_id)
                .execute(connection)?;
                sql_query("INSERT INTO no_intro_dump_details(dump_source_id,source_order,source_line,source_column,opening_end_line,opening_end_column) VALUES(?,0,1,1,1,2)")
                .bind::<BigInt, _>(set_id)
                .execute(connection)?;
                sql_query("INSERT INTO no_intro_dump_serials(dump_source_id,source_order,source_line,source_column) VALUES(?,0,1,1)")
                .bind::<BigInt, _>(set_id)
                .execute(connection)?;
                sql_query("INSERT INTO no_intro_releases(release_id,set_id,source_order,source_line,source_column) VALUES(?,?,1,1,1)")
                .bind::<BigInt, _>(set_id)
                .bind::<BigInt, _>(set_id)
                .execute(connection)?;
                sql_query("INSERT INTO no_intro_release_details(release_id,source_order,source_line,source_column,opening_end_line,opening_end_column) VALUES(?,0,1,1,1,2)")
                .bind::<BigInt, _>(set_id)
                .execute(connection)?;
                sql_query("INSERT INTO no_intro_release_serials(release_id,source_order,source_line,source_column) VALUES(?,0,1,1)")
                .bind::<BigInt, _>(set_id)
                .execute(connection)?;
                sql_query("INSERT INTO asset_occurrences(occurrence_id,record_id,occurrence_order,claim_kind) VALUES(?,?,0,'no_intro_database_source_file')")
                .bind::<BigInt, _>(set_id * 2)
                .bind::<BigInt, _>(set_id)
                .execute(connection)?;
                sql_query("INSERT INTO no_intro_dump_files(occurrence_id,dump_source_id,set_id,source_order,source_line,source_column) VALUES(?,?,?,0,1,1)")
                .bind::<BigInt, _>(set_id * 2)
                .bind::<BigInt, _>(set_id)
                .bind::<BigInt, _>(set_id)
                .execute(connection)?;
                sql_query("INSERT INTO no_intro_dump_file_field_positions(occurrence_id,field_kind,source_order,source_line,source_column) VALUES(?,0,0,1,1)")
                .bind::<BigInt, _>(set_id * 2)
                .execute(connection)?;
                sql_query("INSERT INTO asset_occurrences(occurrence_id,record_id,occurrence_order,claim_kind) VALUES(?,?,1,'no_intro_database_release_file')")
                .bind::<BigInt, _>(set_id * 2 + 1)
                .bind::<BigInt, _>(set_id)
                .execute(connection)?;
                sql_query("INSERT INTO no_intro_release_files(occurrence_id,release_id,set_id,source_order,source_line,source_column) VALUES(?,?,?,0,1,1)")
                .bind::<BigInt, _>(set_id * 2 + 1)
                .bind::<BigInt, _>(set_id)
                .bind::<BigInt, _>(set_id)
                .execute(connection)?;
                sql_query("INSERT INTO no_intro_release_file_field_positions(occurrence_id,field_kind,source_order,source_line,source_column) VALUES(?,0,0,1,1)")
                .bind::<BigInt, _>(set_id * 2 + 1)
                .execute(connection)?;
                sql_query("INSERT INTO no_intro_dump_details_field_positions(dump_source_id,field_kind,source_order,source_line,source_column) VALUES(?,0,0,1,1)")
                .bind::<BigInt, _>(set_id)
                .execute(connection)?;
                sql_query("INSERT INTO no_intro_dump_serials_field_positions(dump_source_id,field_kind,source_order,source_line,source_column) VALUES(?,0,0,1,1)")
                .bind::<BigInt, _>(set_id)
                .execute(connection)?;
                sql_query("INSERT INTO no_intro_release_details_field_positions(release_id,field_kind,source_order,source_line,source_column) VALUES(?,0,0,1,1)")
                .bind::<BigInt, _>(set_id)
                .execute(connection)?;
                sql_query("INSERT INTO no_intro_release_serials_field_positions(release_id,field_kind,source_order,source_line,source_column) VALUES(?,0,0,1,1)")
                .bind::<BigInt, _>(set_id)
                .execute(connection)?;
            }
            sql_query("ANALYZE").execute(connection)?;
            Ok(())
        })
    }

    fn populate_query_provenance(connection: &mut SqliteConnection) -> TestResult {
        connection.transaction(|connection| {
        for set_id in 1_i64..=512 {
            let snapshot = format!("unrelated-snapshot-{set_id}");
            let interpretation = format!("plan-interpretation-{set_id}");
            sql_query("INSERT INTO parser_interpretations(interpretation_key,format) VALUES(?,'no-intro-database-xml-compatible')")
                .bind::<Text, _>(&interpretation)
                .execute(connection)?;
            sql_query("INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) VALUES(?,'plan-catalog','plan-document',?,1)")
                .bind::<Text, _>(&snapshot)
                .bind::<Text, _>(&interpretation)
                .execute(connection)?;
            sql_query("INSERT INTO no_intro_exports(snapshot_key,envelope_kind,header_present,source_line,source_column) VALUES(?,'single_datafile',0,1,1)")
                .bind::<Text, _>(&snapshot)
                .execute(connection)?;
            populate_archive_registry(connection, set_id, &snapshot)?;
            populate_nfo_values(connection, set_id)?;
        }
        sql_query("ANALYZE").execute(connection)?;
        Ok(())
        })
    }

    fn populate_archive_registry(
        connection: &mut SqliteConnection,
        archive_id: i64,
        snapshot: &str,
    ) -> TestResult {
        for (offset, kind, table, column, field) in [
            (
                0,
                "clone",
                "no_intro_archive_clone_links",
                "declared_target_number",
                30,
            ),
            (
                1,
                "mergeof",
                "no_intro_archive_merge_links",
                "declared_mergeof",
                31,
            ),
        ] {
            let relationship_id = archive_id * 2 + offset;
            sql_query("INSERT INTO catalog_relationships(relationship_id,assertion_key,origin,snapshot_key) VALUES(?,?,'source',?)")
                .bind::<BigInt, _>(relationship_id)
                .bind::<Text, _>(format!("plan-relationship-{relationship_id}"))
                .bind::<Text, _>(snapshot)
                .execute(connection)?;
            sql_query("INSERT INTO reported_catalog_relationships(relationship_id,source_reference_kind) VALUES(?,?)")
                .bind::<BigInt, _>(relationship_id)
                .bind::<Text, _>(format!("no_intro_database_archive_{kind}"))
                .execute(connection)?;
            sql_query(format!(
                "INSERT INTO {table}(archive_id,relationship_id,{column}) VALUES(?,?,'literal')"
            ))
            .bind::<BigInt, _>(archive_id)
            .bind::<BigInt, _>(relationship_id)
            .execute(connection)?;
            sql_query("INSERT INTO no_intro_archive_field_positions(archive_id,field_kind,source_order,source_line,source_column) VALUES(?,?,?,1,1)")
                .bind::<BigInt, _>(archive_id)
                .bind::<BigInt, _>(field)
                .bind::<BigInt, _>(field)
                .execute(connection)?;
        }
        Ok(())
    }

    fn populate_nfo_values(connection: &mut SqliteConnection, release_id: i64) -> TestResult {
        let bytes = u32::try_from(release_id)?.to_be_bytes();
        sql_query("INSERT INTO digest_values(digest_id,algorithm,digest) VALUES(?,'crc32',?)")
            .bind::<BigInt, _>(release_id)
            .bind::<diesel::sql_types::Binary, _>(bytes.as_slice())
            .execute(connection)?;
        for (field, code) in [("nfo_crc32", 7), ("nfocrc", 9)] {
            sql_query("INSERT INTO no_intro_release_details_field_positions(release_id,field_kind,source_order,source_line,source_column) VALUES(?,?,?,1,1)")
                .bind::<BigInt, _>(release_id)
                .bind::<BigInt, _>(code)
                .bind::<BigInt, _>(code)
                .execute(connection)?;
            sql_query("INSERT INTO no_intro_release_nfo_hashes(release_id,source_hash_field,hash_id,presence,scope) VALUES(?,?,?,'present','nfo_companion')")
                .bind::<BigInt, _>(release_id)
                .bind::<Text, _>(field)
                .bind::<BigInt, _>(release_id)
                .execute(connection)?;
        }
        for (table, positions, occurrence_id) in [
            (
                "no_intro_dump_file_digests",
                "no_intro_dump_file_field_positions",
                release_id * 2,
            ),
            (
                "no_intro_release_file_digests",
                "no_intro_release_file_field_positions",
                release_id * 2 + 1,
            ),
        ] {
            sql_query(format!("INSERT INTO {positions}(occurrence_id,field_kind,source_order,source_line,source_column) VALUES(?,1,1,1,1)"))
                .bind::<BigInt, _>(occurrence_id)
                .execute(connection)?;
            sql_query(format!(
                "INSERT INTO {table}(occurrence_id,field_kind,digest_id) VALUES(?,0,?)"
            ))
            .bind::<BigInt, _>(occurrence_id)
            .bind::<BigInt, _>(release_id)
            .execute(connection)?;
        }
        Ok(())
    }

    #[test]
    fn actual_archive_child_nfo_and_file_closure_queries_use_owner_indexes() -> TestResult {
        let directory = tempfile::tempdir()?;
        let path = Utf8PathBuf::try_from(directory.path().join("no-intro-native-plan.sqlite"))?;
        let database = Database::open(&path)?;
        let mut connection = database.pool().get()?;
        populate_native_owners(&mut connection)?;
        populate_query_provenance(&mut connection)?;

        let archive_sql = super::archive_rows(2);
        for alias in [
            "native",
            "marker",
            "clone_link",
            "merge_link",
            "clone_reported",
            "clone_registry",
            "merge_reported",
            "merge_registry",
            "game",
            "sets",
            "groups",
            "export",
            "snapshot",
            "interpretation",
        ] {
            assert_indexed_seek(&mut connection, &archive_sql, 2, alias)?;
        }
        let children_sql = super::child_rows(2);
        assert_indexed_seek(&mut connection, &children_sql, 2, "native")?;
        let child_plan = explain(&mut connection, &children_sql, 2)?;
        assert_eq!(
            child_plan
                .iter()
                .filter(|step| step.starts_with("SEARCH native "))
                .count(),
            3,
            "every child UNION branch must seek its game index: {child_plan:?}"
        );
        let nfo_sql = super::nfo_hashes(2);
        for alias in ["native", "digests"] {
            assert_indexed_seek(&mut connection, &nfo_sql, 2, alias)?;
        }
        let closure_sql = super::selected_file_owners(2);
        for alias in [
            "occurrence",
            "source_file",
            "release_file",
            "source_owner",
            "release_owner",
            "owner",
            "file",
            "source_positions",
            "source_digests",
            "release_positions",
            "release_digests",
        ] {
            assert_indexed_seek(&mut connection, &closure_sql, 2, alias)?;
        }
        let closure_plan = explain(&mut connection, &closure_sql, 2)?;
        for index in [
            "no_intro_dump_files_game_lookup",
            "no_intro_release_files_game_lookup",
        ] {
            assert!(
                closure_plan
                    .iter()
                    .any(|step| step.starts_with("SEARCH file ") && step.contains(index)),
                "reverse file ownership must seek its selected-game index {index}: {closure_plan:?}"
            );
        }
        for (alias, expected) in [("file", 2), ("owner", 2), ("occurrence", 3)] {
            assert_eq!(
                closure_plan
                    .iter()
                    .filter(|step| step.starts_with(&format!("SEARCH {alias} ")))
                    .count(),
                expected,
                "every closure branch must seek {alias}: {closure_plan:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn published_game_and_position_queries_use_owner_indexes() -> TestResult {
        let directory = tempfile::tempdir()?;
        let path = Utf8PathBuf::try_from(directory.path().join("no-intro-query-plan.sqlite"))?;
        let database = Database::open(&path)?;
        let mut connection = database.pool().get()?;

        populate_native_owners(&mut connection)?;

        for (sql, parameters) in [
            (games(false), 2),
            (games(true), 4),
            (cursor_anchor().to_owned(), 3),
        ] {
            assert_indexed_seek(&mut connection, &sql, parameters, "sets")?;
            assert_indexed_seek(&mut connection, &sql, parameters, "native")?;
        }

        for owner in [FileOwner::DumpSource, FileOwner::Release] {
            let sql = file_rows(owner, 2);
            for alias in ["owner", "native", "occurrence", "d", "v"] {
                assert_indexed_seek(&mut connection, &sql, 2, alias)?;
            }
        }

        let archive_query = owners(
            "native.source_order",
            "no_intro_archive_descriptions",
            "archive_id",
            2,
            "native.source_order",
        );
        assert_indexed_seek(&mut connection, &archive_query, 2, "native")?;

        for (table, owner) in [
            ("no_intro_dump_sources", "dump_source_id"),
            ("no_intro_dump_details", "dump_source_id"),
            ("no_intro_dump_serials", "dump_source_id"),
            ("no_intro_dump_files", "dump_source_id"),
            ("no_intro_releases", "release_id"),
            ("no_intro_release_details", "release_id"),
            ("no_intro_release_serials", "release_id"),
            ("no_intro_release_files", "release_id"),
        ] {
            let query = owners(
                "native.source_order",
                table,
                owner,
                2,
                "native.source_order",
            );
            assert_indexed_seek(&mut connection, &query, 2, "native")?;
        }

        for (table, owner) in [
            ("no_intro_archive_field_positions", "archive_id"),
            ("no_intro_dump_details_field_positions", "dump_source_id"),
            ("no_intro_dump_serials_field_positions", "dump_source_id"),
            ("no_intro_dump_file_field_positions", "occurrence_id"),
            ("no_intro_release_details_field_positions", "release_id"),
            ("no_intro_release_serials_field_positions", "release_id"),
            ("no_intro_release_file_field_positions", "occurrence_id"),
        ] {
            let query = position_rows(table, owner, 2);
            assert_indexed_seek(&mut connection, &query, 2, "native")?;
        }

        let analyzed = sql_query("SELECT COUNT(*) AS value FROM sqlite_stat1")
            .get_result::<ValueRow>(&mut connection)?
            .value;
        assert!(analyzed > 0, "ANALYZE did not populate SQLite statistics");
        Ok(())
    }
}
