use diesel::{
    connection::{AnsiTransactionManager, TransactionManager},
    result::{Error as DieselError, QueryResult},
    sqlite::SqliteConnection,
};

use crate::domain::ImportRunKey;

#[cfg(test)]
mod diesel_adapter_tests;
#[cfg(test)]
mod tests;

/// The narrow transaction operations used by the import kernel.
///
/// Kept injectable so control-flow outcomes can be tested without claiming
/// that a synthetic database error proves SQLite's native transaction state.
pub(crate) trait TransactionDriver {
    fn begin_immediate(&mut self, conn: &mut SqliteConnection) -> QueryResult<()>;
    fn commit(&mut self, conn: &mut SqliteConnection) -> QueryResult<()>;
    fn rollback(&mut self, conn: &mut SqliteConnection) -> QueryResult<()>;
}

/// Diesel's SQLite transaction-manager adapter for the import kernel.
pub(crate) struct DieselTransactionDriver;

impl TransactionDriver for DieselTransactionDriver {
    fn begin_immediate(&mut self, conn: &mut SqliteConnection) -> QueryResult<()> {
        AnsiTransactionManager::begin_transaction_sql(conn, "BEGIN IMMEDIATE")
    }

    fn commit(&mut self, conn: &mut SqliteConnection) -> QueryResult<()> {
        <AnsiTransactionManager as TransactionManager<SqliteConnection>>::commit_transaction(conn)
    }

    fn rollback(&mut self, conn: &mut SqliteConnection) -> QueryResult<()> {
        <AnsiTransactionManager as TransactionManager<SqliteConnection>>::rollback_transaction(conn)
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum TransactionFailure<E> {
    Operation(E),
    Database(DieselError),
}

#[derive(Debug, PartialEq)]
pub(crate) enum TransactionSecondaryError {
    Rollback(DieselError),
}

#[derive(Debug, PartialEq)]
#[must_use]
pub(crate) enum TransactionOutcome<T, E> {
    Committed {
        run_key: ImportRunKey,
        value: T,
    },
    ConfirmedRollback {
        run_key: ImportRunKey,
        primary: TransactionFailure<E>,
    },
    Unresolved {
        run_key: ImportRunKey,
        primary: TransactionFailure<E>,
        secondary: Vec<TransactionSecondaryError>,
    },
}

/// Run one import attempt without replaying its body.
///
/// A commit is reported only when Diesel reports commit success. A rollback is
/// confirmed only when an explicit rollback call succeeds; otherwise both the
/// original failure and rollback failure are retained as unresolved.
#[must_use]
pub(crate) fn run_with_driver<T, E, D, F>(
    conn: &mut SqliteConnection,
    driver: &mut D,
    work: F,
) -> TransactionOutcome<T, E>
where
    D: TransactionDriver,
    F: FnOnce(&mut SqliteConnection, ImportRunKey) -> Result<T, E>,
{
    let run_key = ImportRunKey::fresh();

    if let Err(error) = driver.begin_immediate(conn) {
        return TransactionOutcome::Unresolved {
            run_key,
            primary: TransactionFailure::Database(error),
            secondary: Vec::new(),
        };
    }

    match work(conn, run_key) {
        Ok(value) => match driver.commit(conn) {
            Ok(()) => TransactionOutcome::Committed { run_key, value },
            Err(error) => {
                rollback_after_failure(conn, driver, run_key, TransactionFailure::Database(error))
            }
        },
        Err(error) => {
            rollback_after_failure(conn, driver, run_key, TransactionFailure::Operation(error))
        }
    }
}

fn rollback_after_failure<T, E, D>(
    conn: &mut SqliteConnection,
    driver: &mut D,
    run_key: ImportRunKey,
    primary: TransactionFailure<E>,
) -> TransactionOutcome<T, E>
where
    D: TransactionDriver,
{
    match driver.rollback(conn) {
        Ok(()) => TransactionOutcome::ConfirmedRollback { run_key, primary },
        Err(error) => TransactionOutcome::Unresolved {
            run_key,
            primary,
            secondary: vec![TransactionSecondaryError::Rollback(error)],
        },
    }
}
