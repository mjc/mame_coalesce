//! Shared accepted ROM facts, independent of the number of source declarations.

use diesel::{RunQueryDsl, SqliteConnection};

use crate::{
    domain::OccurrenceId,
    storage::cached_sql::{OwnedBinding, cached_generated_sql},
};

/// Add facts only after these native declarations and their evidence are complete.
///
/// Imports are append-only. Reviews can withdraw evidence, so their publication
/// trigger rebuilds the affected components from surviving source witnesses.
pub fn record_occurrences(
    connection: &mut SqliteConnection,
    occurrences: &[OccurrenceId],
) -> crate::Result<()> {
    for page in occurrences.chunks(128) {
        let arity = page.len().next_power_of_two();
        let bindings = (0..arity)
            .map(|index| OwnedBinding::BigInt(page[index.min(page.len() - 1)].database_value()))
            .collect::<Vec<_>>();
        let values = std::iter::repeat_n("(?)", arity)
            .collect::<Vec<_>>()
            .join(",");
        let requested = format!("WITH requested(occurrence_id) AS (VALUES {values}) ");
        cached_generated_sql(
            format!(
                "{requested} INSERT OR IGNORE INTO shared_file_sizes(content_uuid,size) \
             SELECT DISTINCT canonical.content_uuid,sizes.size FROM requested \
             CROSS JOIN canonical_occurrence_content AS canonical USING(occurrence_id) \
             CROSS JOIN accepted_file_size_assertions AS sizes USING(occurrence_id) \
             WHERE canonical.content_uuid IS NOT NULL"
            ),
            bindings.clone(),
        )
        .execute(connection)?;
        cached_generated_sql(
            format!(
                "{requested} INSERT OR IGNORE INTO shared_file_hashes(content_uuid,digest_id) \
             SELECT DISTINCT assertion.content_uuid,assertion.digest_id FROM requested \
             CROSS JOIN catalog_content_digest_assertions AS assertion USING(occurrence_id) \
             WHERE assertion.content_uuid IS NOT NULL"
            ),
            bindings,
        )
        .execute(connection)?;
    }
    Ok(())
}
