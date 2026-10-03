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
SELECT set_group_id, kind, list_order FROM catalog_set_groups
WHERE snapshot_key = ? ORDER BY kind, list_order LIMIT 2";

pub(super) const GAME_COLUMNS: &str = "
SELECT sets.set_id, sets.set_name, sets.list_order, sets.source_element_kind,
       sets.source_line AS line, sets.source_column AS column,
       native.set_id AS native_id, native.source_file, native.is_bios,
       native.is_bios_was_present, native.board, native.rebuild_to,
       native.description, native.year, native.manufacturer,
       typeof(sets.list_order)='integer' AND typeof(sets.source_line)='integer'
       AND typeof(sets.source_column)='integer' AND typeof(native.is_bios_was_present)='integer' AS valid
FROM catalog_sets AS sets LEFT JOIN logiqx_games AS native ON native.set_id = sets.set_id
WHERE sets.set_group_id = ?";

pub(super) fn games(after_cursor: bool) -> String {
    let continuation = if after_cursor {
        " AND (sets.list_order, sets.set_id) > (?, ?)"
    } else {
        ""
    };
    format!("{GAME_COLUMNS}{continuation} ORDER BY sets.list_order, sets.set_id LIMIT ?")
}

/// The requested CTE drives primary-key seeks for each bounded game owner.
pub(super) fn owners(select: &str, from: &str, count: usize, order: &str) -> String {
    let values = std::iter::repeat_n("(?)", count)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "WITH requested(owner_id) AS (VALUES {values}) \
         SELECT native.set_id AS owner_id, {select} \
         FROM requested CROSS JOIN {from} \
         WHERE native.set_id = requested.owner_id ORDER BY native.set_id, {order}"
    )
}

/// Test SQLite's stored types before Diesel's coercing integer deserialization.
pub(super) fn integer_columns(columns: &[&str]) -> String {
    columns
        .iter()
        .map(|column| format!("typeof({column})='integer'"))
        .collect::<Vec<_>>()
        .join(" AND ")
}

pub(super) fn children(
    select: &str,
    table: &str,
    order: &str,
    presence: Option<&str>,
    count: usize,
) -> String {
    let mut columns = vec![
        order,
        "native.source_order",
        "native.source_line",
        "native.source_column",
    ];
    columns.extend(presence);
    owners(
        &format!("{select}, {} AS valid", integer_columns(&columns)),
        &format!("{table} AS native"),
        count,
        order,
    )
}

pub(super) fn document(select: &str, table: &str, integers: &[&str]) -> String {
    format!(
        "SELECT {select}, {} AS valid FROM {table} WHERE snapshot_key = ?",
        integer_columns(integers)
    )
}

pub(super) fn positions(table: &str, child_key: Option<&str>, count: usize) -> String {
    let child = child_key.unwrap_or("0");
    owners(
        &format!(
            "{child} AS child_order, native.field_kind, native.source_order, \
                  native.source_line AS line, native.source_column AS column, \
                  typeof(native.field_kind)='integer' AND typeof(native.source_order)='integer' \
                  AND typeof(native.source_line)='integer' AND typeof(native.source_column)='integer' \
                  AND typeof({child})='integer' AS valid"
        ),
        &format!("{table} AS native"),
        count,
        "child_order, native.source_order",
    )
}

pub(super) fn document_positions(table: &str) -> String {
    format!(
        "SELECT 0 AS owner_id, 0 AS child_order, field_kind, source_order, \
         source_line AS line, source_column AS column, \
         typeof(field_kind)='integer' AND typeof(source_order)='integer' \
         AND typeof(source_line)='integer' AND typeof(source_column)='integer' AS valid \
         FROM {table} WHERE snapshot_key = ? ORDER BY source_order"
    )
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use diesel::{
        QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
        sql_types::{BigInt, Text},
    };

    use crate::database::Database;

    use super::{GROUPS, SNAPSHOT, document_positions, games, owners, positions};

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    #[derive(QueryableByName)]
    struct PlanRow {
        #[diesel(sql_type = Text)]
        detail: String,
    }

    fn assert_indexed(
        connection: &mut SqliteConnection,
        query: &str,
        parameters: usize,
        alias: &str,
    ) -> TestResult {
        let mut query =
            sql_query(format!("EXPLAIN QUERY PLAN {query}")).into_boxed::<diesel::sqlite::Sqlite>();
        for _ in 0..parameters {
            query = query.bind::<BigInt, _>(1);
        }
        let plan = query
            .load::<PlanRow>(connection)?
            .into_iter()
            .map(|row| row.detail)
            .collect::<Vec<_>>();
        assert!(
            plan.iter()
                .any(|step| step.starts_with(&format!("SEARCH {alias} "))),
            "missing indexed seek for {alias}: {plan:?}"
        );
        assert!(
            !plan
                .iter()
                .any(|step| step.starts_with(&format!("SCAN {alias}"))),
            "unbounded owner scan for {alias}: {plan:?}"
        );
        Ok(())
    }

    #[test]
    fn native_page_queries_seek_bounded_owner_indexes() -> TestResult {
        let directory = tempfile::tempdir()?;
        let path = Utf8PathBuf::try_from(directory.path().join("query-plans.sqlite"))?;
        let database = Database::open(&path)?;
        let mut connection = database.pool().get()?;
        assert_indexed(&mut connection, SNAPSHOT, 1, "snapshots")?;
        assert_indexed(&mut connection, GROUPS, 1, "catalog_set_groups")?;
        assert_indexed(&mut connection, &games(false), 2, "sets")?;
        assert_indexed(&mut connection, &games(true), 4, "sets")?;
        for table in [
            "logiqx_game_comments",
            "logiqx_releases",
            "logiqx_bios_sets",
            "logiqx_archive_references",
            "logiqx_device_references",
            "logiqx_set_links",
        ] {
            assert_indexed(
                &mut connection,
                &owners(
                    "1 AS value",
                    &format!("{table} AS native"),
                    2,
                    "native.set_id",
                ),
                2,
                "native",
            )?;
        }
        assert_position_plans(&mut connection)
    }

    fn assert_position_plans(connection: &mut SqliteConnection) -> TestResult {
        for (table, child) in [
            ("logiqx_game_attribute_positions", None),
            ("logiqx_game_text_positions", None),
            (
                "logiqx_release_attribute_positions",
                Some("native.release_order"),
            ),
            ("logiqx_bios_attribute_positions", Some("native.bios_order")),
            (
                "logiqx_archive_attribute_positions",
                Some("native.archive_order"),
            ),
            (
                "logiqx_device_reference_attribute_positions",
                Some("native.reference_order"),
            ),
        ] {
            assert_indexed(connection, &positions(table, child, 2), 2, "native")?;
        }
        for table in [
            "logiqx_document_attribute_positions",
            "logiqx_header_text_positions",
            "logiqx_clrmamepro_attribute_positions",
            "logiqx_romcenter_attribute_positions",
        ] {
            assert_indexed(connection, &document_positions(table), 1, table)?;
        }
        assert_indexed(connection, &super::super::media::select(2), 2, "native")?;
        for table in [
            "logiqx_rom_attribute_positions",
            "logiqx_disk_attribute_positions",
            "logiqx_sample_attribute_positions",
        ] {
            let query = super::super::media::attribute_select(table, 2);
            assert_indexed(connection, &query, 2, "occurrences")?;
            assert_indexed(connection, &query, 2, "native")?;
        }
        Ok(())
    }
}
