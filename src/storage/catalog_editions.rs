//! Edition selection and fresh attempts for a fixed catalog interpretation.

use diesel::{
    Connection, OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};

use super::catalog_ids::{
    CatalogId, CoverageId, EditionId, FileReceiptId, ImportId, ReadingRulesId, SourceFileId,
};

/// Fixed source and interpretation selected before parsing begins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditionTarget {
    pub catalog_id: CatalogId,
    pub source_file_id: SourceFileId,
    pub reading_rules_id: ReadingRulesId,
    pub coverage_id: CoverageId,
    /// This attempt's receipt; reimport does not change an existing edition's receipt.
    pub file_receipt_id: Option<FileReceiptId>,
}

/// New native facts versus verification of an immutable publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditionSelection {
    New(EditionId),
    Reused(EditionId),
}

impl EditionSelection {
    /// Exact edition selected for this attempt, never a later latest lookup.
    #[must_use]
    pub const fn id(self) -> EditionId {
        match self {
            Self::New(id) | Self::Reused(id) => id,
        }
    }
}

/// Metadata for a running import; this is not a parser acceptance capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartedImport {
    pub import_id: ImportId,
    pub edition: EditionSelection,
}

#[derive(QueryableByName)]
struct ExistingEdition {
    #[diesel(sql_type = BigInt)]
    edition_id: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    published_id: Option<i64>,
}

#[derive(QueryableByName)]
struct InsertedEdition {
    #[diesel(sql_type = BigInt)]
    edition_id: i64,
}

#[derive(QueryableByName)]
struct InsertedImport {
    #[diesel(sql_type = BigInt)]
    import_id: i64,
}

/// Select one edition and start a fresh attempt atomically.
///
/// The import driver calls this within its immediate transaction, which also
/// owns all native writes, EOF validation and publication. The nested savepoint
/// ensures a failed attempt insert cannot strand a draft even when its error is
/// caught by the caller. A draft belonging to another attempt is never reused.
/// This operation does not publish or certify the source in either branch.
pub fn begin(
    connection: &mut SqliteConnection,
    target: EditionTarget,
    import_key: &str,
    started_at: &str,
) -> crate::Result<StartedImport> {
    connection.transaction(|connection| {
        let existing = sql_query(
            "SELECT edition.edition_id, publication.edition_id AS published_id \
             FROM catalog_editions AS edition \
             LEFT JOIN published_catalog_editions AS publication USING (edition_id) \
             WHERE edition.catalog_id = ? AND edition.source_file_id = ? \
               AND edition.reading_rules_id = ? AND edition.coverage_id = ?",
        )
        .bind::<BigInt, _>(target.catalog_id.as_i64())
        .bind::<BigInt, _>(target.source_file_id.as_i64())
        .bind::<BigInt, _>(target.reading_rules_id.as_i64())
        .bind::<BigInt, _>(target.coverage_id.as_i64())
        .get_result::<ExistingEdition>(connection)
        .optional()?;
        let edition = match existing {
            Some(existing) if existing.published_id.is_some() => {
                EditionSelection::Reused(checked_id(existing.edition_id)?)
            }
            Some(existing) => {
                return Err(crate::Error::DatabaseSchema(format!(
                    "edition {} is an unpublished draft owned by another attempt",
                    existing.edition_id
                )));
            }
            None => {
                let inserted = sql_query(
                    "INSERT INTO catalog_editions \
                     (catalog_id, source_file_id, reading_rules_id, coverage_id, \
                      file_receipt_id, previous_edition_id) \
                     VALUES (?, ?, ?, ?, ?, \
                        (SELECT edition_id FROM published_catalog_editions \
                         WHERE catalog_id = ? ORDER BY published_at DESC, edition_id DESC LIMIT 1)) \
                     RETURNING edition_id",
                )
                .bind::<BigInt, _>(target.catalog_id.as_i64())
                .bind::<BigInt, _>(target.source_file_id.as_i64())
                .bind::<BigInt, _>(target.reading_rules_id.as_i64())
                .bind::<BigInt, _>(target.coverage_id.as_i64())
                .bind::<Nullable<BigInt>, _>(target.file_receipt_id.map(FileReceiptId::as_i64))
                .bind::<BigInt, _>(target.catalog_id.as_i64())
                .get_result::<InsertedEdition>(connection)?;
                EditionSelection::New(checked_id(inserted.edition_id)?)
            }
        };
        let inserted = sql_query(
            "INSERT INTO catalog_imports \
             (import_key, catalog_id, source_file_id, reading_rules_id, \
              file_receipt_id, edition_id, status, started_at) \
             VALUES (?, ?, ?, ?, ?, ?, 'running', ?) RETURNING import_id",
        )
        .bind::<Text, _>(import_key)
        .bind::<BigInt, _>(target.catalog_id.as_i64())
        .bind::<BigInt, _>(target.source_file_id.as_i64())
        .bind::<BigInt, _>(target.reading_rules_id.as_i64())
        .bind::<Nullable<BigInt>, _>(target.file_receipt_id.map(FileReceiptId::as_i64))
        .bind::<BigInt, _>(edition.id().as_i64())
        .bind::<Text, _>(started_at)
        .get_result::<InsertedImport>(connection)?;
        Ok(StartedImport {
            import_id: checked_id(inserted.import_id)?,
            edition,
        })
    })
}

fn checked_id<Id>(value: i64) -> crate::Result<Id>
where
    Id: TryFrom<i64, Error = super::catalog_ids::CatalogIdError>,
{
    Id::try_from(value).map_err(|error| crate::Error::DatabaseSchema(error.to_string()))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::storage::catalog_ids::{CatalogId, CoverageId, ReadingRulesId, SourceFileId};
    use diesel::{
        Connection, QueryableByName, RunQueryDsl, SqliteConnection, connection::SimpleConnection,
        sql_query, sql_types::BigInt,
    };

    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = BigInt)]
        count: i64,
    }

    fn fixture() -> crate::Result<SqliteConnection> {
        let mut conn = SqliteConnection::establish(":memory:")
            .map_err(|error| crate::Error::DatabaseSchema(error.to_string()))?;
        crate::storage::db::initialize_database(&mut conn)?;
        conn.batch_execute(
            "INSERT INTO catalog_publishers VALUES(1,'publisher','Publisher',NULL);
             INSERT INTO catalogs VALUES(1,1,'catalog','Catalog');
             INSERT INTO catalog_source_files VALUES(1,zeroblob(32),NULL,64,'sha256/source','zstd');
             INSERT INTO catalog_reading_rules VALUES(1,'rules','mame','observed','0.289','test','v1');
             INSERT INTO catalog_coverage VALUES(1,'complete');"
        )?;
        Ok(conn)
    }

    fn target() -> EditionTarget {
        EditionTarget {
            catalog_id: CatalogId::try_from(1).expect("positive catalog ID"),
            source_file_id: SourceFileId::try_from(1).expect("positive source ID"),
            reading_rules_id: ReadingRulesId::try_from(1).expect("positive rules ID"),
            coverage_id: CoverageId::try_from(1).expect("positive coverage ID"),
            file_receipt_id: None,
        }
    }

    #[test]
    fn a_new_edition_starts_a_running_attempt_without_publishing() -> crate::Result<()> {
        let mut conn = fixture()?;
        conn.immediate_transaction(|conn| {
            let started = begin(conn, target(), "attempt", "2026-10-08T00:00:00Z")?;
            assert!(matches!(started.edition, EditionSelection::New(_)));
            let row = sql_query("SELECT count(*) AS count FROM catalog_imports WHERE status='running' AND finished_at IS NULL")
                .get_result::<Count>(conn)?;
            assert_eq!(row.count, 1);
            let row = sql_query("SELECT count(*) AS count FROM published_catalog_editions")
                .get_result::<Count>(conn)?;
            assert_eq!(row.count, 0);
            Ok(())
        })
    }

    #[test]
    fn an_unrelated_draft_is_not_reused_or_mutated() -> crate::Result<()> {
        let mut conn = fixture()?;
        conn.immediate_transaction(|conn| {
            let first = begin(conn, target(), "first", "2026-10-08T00:00:00Z")?;
            assert!(begin(conn, target(), "second", "2026-10-08T00:00:01Z").is_err());
            let row = sql_query("SELECT count(*) AS count FROM catalog_imports")
                .get_result::<Count>(conn)?;
            assert_eq!(row.count, 1);
            assert!(first.import_id.as_i64() > 0);
            Ok(())
        })
    }

    #[test]
    fn rollback_removes_the_new_edition_and_attempt_together() -> crate::Result<()> {
        let mut conn = fixture()?;
        let result = conn.immediate_transaction::<(), crate::Error, _>(|conn| {
            begin(conn, target(), "rolled-back", "2026-10-08T00:00:00Z")?;
            Err(crate::Error::DatabaseSchema(
                "injected parser rejection".to_owned(),
            ))
        });
        assert!(
            matches!(result, Err(crate::Error::DatabaseSchema(ref message))
            if message == "injected parser rejection")
        );
        for table in ["catalog_editions", "catalog_imports"] {
            let row = sql_query(format!("SELECT count(*) AS count FROM {table}"))
                .get_result::<Count>(&mut conn)?;
            assert_eq!(row.count, 0);
        }
        Ok(())
    }

    #[test]
    fn reimport_reuses_the_exact_publication_with_a_fresh_attempt() -> crate::Result<()> {
        let mut conn = fixture()?;
        conn.immediate_transaction(|conn| {
            let first = begin(conn, target(), "first", "2026-10-08T00:00:00Z")?;
            let EditionSelection::New(edition) = first.edition else {
                panic!("first import must create an edition");
            };
            crate::storage::test_catalog::publish_minimal_mame(
                conn,
                edition,
                "2026-10-08T00:00:01Z",
            )?;
            let second = begin(conn, target(), "second", "2026-10-08T00:00:02Z")?;
            assert_eq!(second.edition, EditionSelection::Reused(edition));
            assert_ne!(first.import_id, second.import_id);
            for table in [
                "catalog_editions",
                "published_catalog_editions",
                "mame_machines",
            ] {
                let row = sql_query(format!("SELECT count(*) AS count FROM {table}"))
                    .get_result::<Count>(conn)?;
                assert_eq!(row.count, 1);
            }
            Ok(())
        })
    }

    #[test]
    fn a_duplicate_attempt_key_does_not_leave_another_draft() -> crate::Result<()> {
        let mut conn = fixture()?;
        conn.immediate_transaction(|conn| {
            begin(conn, target(), "duplicate", "2026-10-08T00:00:00Z")?;
            conn.batch_execute("INSERT INTO catalog_coverage VALUES(2,'unknown')")?;
            let different = EditionTarget {
                coverage_id: CoverageId::try_from(2).expect("positive coverage ID"),
                ..target()
            };
            assert!(begin(conn, different, "duplicate", "2026-10-08T00:00:01Z").is_err());
            let row = sql_query("SELECT count(*) AS count FROM catalog_editions")
                .get_result::<Count>(conn)?;
            assert_eq!(row.count, 1);
            Ok(())
        })
    }
}
