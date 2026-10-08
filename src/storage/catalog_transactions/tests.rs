#![allow(clippy::expect_used)]

use std::collections::VecDeque;

use diesel::{Connection, result::QueryResult, sqlite::SqliteConnection};

use super::{
    TransactionDriver, TransactionFailure, TransactionOutcome, TransactionSecondaryError,
    run_with_driver,
};

#[derive(Default)]
struct ScriptedDriver {
    begin: VecDeque<QueryResult<()>>,
    commits: VecDeque<QueryResult<()>>,
    rollbacks: VecDeque<QueryResult<()>>,
    begin_calls: usize,
    commit_calls: usize,
    rollback_calls: usize,
}

impl TransactionDriver for ScriptedDriver {
    fn begin_immediate(&mut self, _: &mut SqliteConnection) -> QueryResult<()> {
        self.begin_calls += 1;
        self.begin.pop_front().unwrap_or(Ok(()))
    }

    fn commit(&mut self, _: &mut SqliteConnection) -> QueryResult<()> {
        self.commit_calls += 1;
        self.commits.pop_front().unwrap_or(Ok(()))
    }

    fn rollback(&mut self, _: &mut SqliteConnection) -> QueryResult<()> {
        self.rollback_calls += 1;
        self.rollbacks.pop_front().unwrap_or(Ok(()))
    }
}

fn connection() -> SqliteConnection {
    SqliteConnection::establish(":memory:").expect("in-memory SQLite connection")
}

#[test]
fn only_a_successful_commit_returns_committed_and_work_runs_once() {
    let mut conn = connection();
    let mut driver = ScriptedDriver::default();
    let mut work_calls = 0;
    let mut observed_run = None;

    let outcome = run_with_driver(&mut conn, &mut driver, |_, run_key| {
        work_calls += 1;
        observed_run = Some(run_key);
        Ok::<_, &'static str>(17)
    });

    let TransactionOutcome::Committed { run_key, value } = outcome else {
        panic!("successful commit must be the only committed outcome")
    };
    assert_eq!(value, 17);
    assert_eq!(Some(run_key), observed_run);
    assert_eq!(work_calls, 1);
    assert_eq!(driver.begin_calls, 1);
    assert_eq!(driver.commit_calls, 1);
    assert_eq!(driver.rollback_calls, 0);
}

#[test]
fn operation_error_is_confirmed_only_when_explicit_rollback_succeeds() {
    let mut conn = connection();
    let mut driver = ScriptedDriver::default();
    driver.rollbacks.push_back(Ok(()));
    let mut work_calls = 0;

    let outcome = run_with_driver(&mut conn, &mut driver, |_, _| {
        work_calls += 1;
        Err::<(), _>("native write failed")
    });

    let TransactionOutcome::ConfirmedRollback {
        primary: TransactionFailure::Operation(primary),
        ..
    } = outcome
    else {
        panic!("only a successful explicit rollback confirms rollback")
    };
    assert_eq!(primary, "native write failed");
    assert_eq!(work_calls, 1);
    assert_eq!(driver.commit_calls, 0);
    assert_eq!(driver.rollback_calls, 1);
}

#[test]
fn commit_error_is_preserved_when_explicit_rollback_confirms_rollback() {
    let mut conn = connection();
    let mut driver = ScriptedDriver::default();
    driver
        .commits
        .push_back(Err(diesel::result::Error::RollbackTransaction));
    driver.rollbacks.push_back(Ok(()));
    let mut work_calls = 0;

    let outcome = run_with_driver(&mut conn, &mut driver, |_, _| {
        work_calls += 1;
        Ok::<_, &'static str>(())
    });

    let TransactionOutcome::ConfirmedRollback {
        primary: TransactionFailure::Database(primary),
        ..
    } = outcome
    else {
        panic!("failed commit plus successful rollback is confirmed rollback")
    };
    assert!(matches!(
        primary,
        diesel::result::Error::RollbackTransaction
    ));
    assert_eq!(
        work_calls, 1,
        "commit handling must never replay import work"
    );
    assert_eq!(driver.commit_calls, 1);
    assert_eq!(driver.rollback_calls, 1);
}

#[test]
fn commit_and_rollback_errors_remain_unresolved_without_error_text_inference() {
    let mut conn = connection();
    let mut driver = ScriptedDriver::default();
    driver
        .commits
        .push_back(Err(diesel::result::Error::RollbackTransaction));
    driver
        .rollbacks
        .push_back(Err(diesel::result::Error::NotInTransaction));

    let outcome = run_with_driver(&mut conn, &mut driver, |_, _| Ok::<_, &'static str>(()));

    let TransactionOutcome::Unresolved {
        primary: TransactionFailure::Database(primary),
        secondary,
        ..
    } = outcome
    else {
        panic!("a failed rollback cannot be treated as confirmed rollback")
    };
    assert!(matches!(
        primary,
        diesel::result::Error::RollbackTransaction
    ));
    assert!(matches!(
        secondary.as_slice(),
        [TransactionSecondaryError::Rollback(error)]
            if matches!(error, diesel::result::Error::NotInTransaction)
    ));
    assert_eq!(
        driver.commit_calls, 1,
        "unclassified commits are not retried"
    );
    assert_eq!(driver.rollback_calls, 1);
}

#[test]
fn failed_begin_never_invokes_import_work_or_claims_rollback() {
    let mut conn = connection();
    let mut driver = ScriptedDriver::default();
    driver
        .begin
        .push_back(Err(diesel::result::Error::AlreadyInTransaction));
    let mut work_calls = 0;

    let outcome = run_with_driver(&mut conn, &mut driver, |_, _| {
        work_calls += 1;
        Ok::<_, &'static str>(())
    });

    assert!(matches!(
        outcome,
        TransactionOutcome::Unresolved {
            primary: TransactionFailure::Database(diesel::result::Error::AlreadyInTransaction),
            ..
        }
    ));
    assert_eq!(work_calls, 0);
    assert_eq!(driver.commit_calls, 0);
    assert_eq!(driver.rollback_calls, 0);
}

#[test]
fn failed_explicit_rollback_does_not_prove_sqlite_auto_rollback() {
    let mut conn = connection();
    let mut driver = ScriptedDriver::default();
    driver
        .commits
        .push_back(Err(diesel::result::Error::RollbackTransaction));
    driver
        .rollbacks
        .push_back(Err(diesel::result::Error::NotInTransaction));

    let outcome = run_with_driver(&mut conn, &mut driver, |_, _| Ok::<_, &'static str>(()));

    assert!(matches!(outcome, TransactionOutcome::Unresolved { .. }));
}

#[test]
fn each_run_uses_a_distinct_import_run_key() {
    let mut conn = connection();
    let mut driver = ScriptedDriver::default();
    let first = run_with_driver(&mut conn, &mut driver, |_, run_key| {
        let run_key: crate::domain::ImportRunKey = run_key;
        Ok::<_, &'static str>(run_key)
    });
    let second = run_with_driver(&mut conn, &mut driver, |_, run_key| {
        let run_key: crate::domain::ImportRunKey = run_key;
        Ok::<_, &'static str>(run_key)
    });

    let TransactionOutcome::Committed {
        run_key: first_key,
        value: first_value,
    } = first
    else {
        panic!("first run committed")
    };
    let TransactionOutcome::Committed {
        run_key: second_key,
        value: second_value,
    } = second
    else {
        panic!("second run committed")
    };
    assert_eq!(first_key, first_value);
    assert_eq!(second_key, second_value);
    assert_ne!(first_key, second_key);
}
