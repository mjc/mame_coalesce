use diesel::{
    Connection, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Nullable, Text},
};

use super::{ensure, load};
use crate::domain::{
    CatalogScope, ParserInterpretationKey, QualifiedCatalogSet, SetCoverage, SetName,
};

fn connection() -> crate::Result<SqliteConnection> {
    let mut connection = SqliteConnection::establish(":memory:")
        .map_err(|error| crate::Error::DatabaseSchema(error.to_string()))?;
    connection.batch_execute(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE catalog_snapshots (
             snapshot_key TEXT PRIMARY KEY,
             coverage_id INTEGER NOT NULL REFERENCES catalog_coverage(coverage_id)
         );",
    )?;
    connection.batch_execute(include_str!("../db/coverage.sql"))?;
    Ok(connection)
}

fn root(name: &str) -> QualifiedCatalogSet {
    QualifiedCatalogSet::RootSet(SetName::new(name))
}

fn software(list_name: &str, name: &str) -> QualifiedCatalogSet {
    QualifiedCatalogSet::SoftwareItem {
        list_name: list_name.to_owned(),
        name: SetName::new(name),
    }
}

#[derive(Clone, Copy)]
struct MemberRow<'a> {
    coverage_id: i64,
    list_order: i64,
    set_kind: &'a str,
    set_group_kind: &'a str,
    software_list_name: Option<&'a str>,
    set_name: &'a str,
    coverage: &'a str,
}

fn insert_member(
    connection: &mut SqliteConnection,
    row: MemberRow<'_>,
) -> diesel::QueryResult<usize> {
    sql_query(
        "INSERT INTO catalog_covered_sets \
         (coverage_id, list_order, set_kind, set_group_kind, software_list_name, set_name, coverage) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(row.coverage_id)
    .bind::<BigInt, _>(row.list_order)
    .bind::<Text, _>(row.set_kind)
    .bind::<Text, _>(row.set_group_kind)
    .bind::<Nullable<Text>, _>(row.software_list_name)
    .bind::<Text, _>(row.set_name)
    .bind::<Text, _>(row.coverage)
    .execute(connection)
}

#[test]
fn exact_typed_scope_reuses_identity_independent_of_construction_order() -> crate::Result<()> {
    let mut connection = connection()?;
    let first = CatalogScope::Filtered([root("beta"), root("alpha")].into_iter().collect());
    let second = CatalogScope::Filtered([root("alpha"), root("beta")].into_iter().collect());

    connection.immediate_transaction(|connection| {
        let first_id = ensure(connection, &first)?;
        let second_id = ensure(connection, &second)?;
        assert_eq!(first_id, second_id);
        assert_eq!(load(connection, first_id)?, first);
        Ok(())
    })
}

#[test]
fn covered_and_unknown_partial_members_have_distinct_identities() -> crate::Result<()> {
    let mut connection = connection()?;
    let covered = CatalogScope::Partial([(root("game"), SetCoverage::Covered)].into());
    let unknown = CatalogScope::Partial([(root("game"), SetCoverage::Unknown)].into());

    connection.immediate_transaction(|connection| {
        let covered_id = ensure(connection, &covered)?;
        let unknown_id = ensure(connection, &unknown)?;
        assert_ne!(covered_id, unknown_id);
        assert_eq!(load(connection, covered_id)?, covered);
        assert_eq!(load(connection, unknown_id)?, unknown);
        Ok(())
    })
}

#[test]
fn software_items_are_qualified_by_list_and_distinct_from_root_sets() -> crate::Result<()> {
    let mut connection = connection()?;
    let root_scope = CatalogScope::Filtered([root("game")].into());
    let software_scope = CatalogScope::Filtered([software("list", "game")].into());

    connection.immediate_transaction(|connection| {
        let root_id = ensure(connection, &root_scope)?;
        let software_id = ensure(connection, &software_scope)?;
        assert_ne!(root_id, software_id);
        assert_eq!(load(connection, software_id)?, software_scope);
        Ok(())
    })
}

#[test]
fn slash_names_are_not_ambiguous_between_root_and_software_qualification() -> crate::Result<()> {
    let mut connection = connection()?;
    let root_scope = CatalogScope::Filtered([root("list/item")].into());
    let software_scope = CatalogScope::Filtered([software("list", "item")].into());

    connection.immediate_transaction(|connection| {
        let root_id = ensure(connection, &root_scope)?;
        let software_id = ensure(connection, &software_scope)?;
        assert_ne!(root_id, software_id);
        assert_eq!(load(connection, root_id)?, root_scope);
        assert_eq!(load(connection, software_id)?, software_scope);
        Ok(())
    })
}

#[test]
fn parser_interpretation_key_uses_canonical_typed_scope_identity() {
    let root_scope = CatalogScope::Filtered([root("list/item")].into());
    let software_scope = CatalogScope::Filtered([software("list", "item")].into());
    let same_root_scope = CatalogScope::Filtered([root("list/item")].into());

    let root_key = ParserInterpretationKey::for_format("mame", &root_scope);
    assert_eq!(
        root_key,
        ParserInterpretationKey::for_format("mame", &same_root_scope)
    );
    assert_ne!(
        root_key,
        ParserInterpretationKey::for_format("mame", &software_scope)
    );
}

#[test]
fn selected_but_absent_member_is_retained_in_filtered_scope() -> crate::Result<()> {
    let mut connection = connection()?;
    let scope = CatalogScope::Filtered([root("selected-but-not-present")].into());

    connection.immediate_transaction(|connection| {
        let id = ensure(connection, &scope)?;
        assert_eq!(load(connection, id)?, scope);
        Ok(())
    })
}

#[test]
fn root_member_partial_index_rejects_duplicates_with_null_list_name() -> crate::Result<()> {
    let mut connection = connection()?;
    let scope = CatalogScope::Filtered([root("game")].into());

    connection.immediate_transaction(|connection| {
        let id = ensure(connection, &scope)?;
        let duplicate = sql_query(
            "INSERT INTO catalog_covered_sets \
             (coverage_id, list_order, set_kind, set_group_kind, software_list_name, set_name, coverage) \
             VALUES (?, 1, 'root', 'root', NULL, 'game', 'covered')",
        )
        .bind::<BigInt, _>(id.database_value())
        .execute(connection);
        assert!(duplicate.is_err());
        Ok(())
    })
}

#[test]
fn unknown_and_complete_scopes_have_no_member_rows() -> crate::Result<()> {
    let mut connection = connection()?;

    connection.immediate_transaction(|connection| {
        for scope in [CatalogScope::Unknown, CatalogScope::Complete] {
            let id = ensure(connection, &scope)?;
            assert_eq!(load(connection, id)?, scope);
        }
        let count = sql_query("SELECT COUNT(*) AS member_count FROM catalog_covered_sets")
            .get_result::<MemberCount>(connection)?
            .member_count;
        assert_eq!(count, 0);
        Ok(())
    })
}

#[test]
fn sql_guards_reject_members_in_incompatible_scope_kinds() -> crate::Result<()> {
    let mut connection = connection()?;

    connection.immediate_transaction(|connection| {
        let unknown_id = ensure(connection, &CatalogScope::Unknown)?;
        let complete_id = ensure(connection, &CatalogScope::Complete)?;
        let filtered_id = ensure(connection, &CatalogScope::Filtered([root("game")].into()))?;

        for id in [unknown_id, complete_id] {
            assert!(
                insert_member(
                    connection,
                    MemberRow {
                        coverage_id: id.database_value(),
                        list_order: 0,
                        set_kind: "root",
                        set_group_kind: "root",
                        software_list_name: None,
                        set_name: "game",
                        coverage: "covered",
                    },
                )
                .is_err()
            );
        }
        assert!(
            insert_member(
                connection,
                MemberRow {
                    coverage_id: filtered_id.database_value(),
                    list_order: 1,
                    set_kind: "root",
                    set_group_kind: "root",
                    software_list_name: None,
                    set_name: "other",
                    coverage: "unknown",
                },
            )
            .is_err()
        );
        Ok(())
    })
}

#[test]
fn unreferenced_scope_can_be_built_but_referenced_scope_is_immutable() -> crate::Result<()> {
    let mut connection = connection()?;
    let scope = CatalogScope::Filtered([root("game")].into());

    connection.immediate_transaction(|connection| {
        let id = ensure(connection, &scope)?;
        insert_member(
            connection,
            MemberRow {
                coverage_id: id.database_value(),
                list_order: 1,
                set_kind: "root",
                set_group_kind: "root",
                software_list_name: None,
                set_name: "extra",
                coverage: "covered",
            },
        )?;
        connection.batch_execute(&format!(
            "INSERT INTO catalog_snapshots(snapshot_key, coverage_id) VALUES ('snapshot', {})",
            id.database_value()
        ))?;

        assert!(
            sql_query("UPDATE catalog_coverage SET kind = 'partial' WHERE coverage_id = ?")
                .bind::<BigInt, _>(id.database_value())
                .execute(connection)
                .is_err()
        );
        assert!(
            insert_member(
                connection,
                MemberRow {
                    coverage_id: id.database_value(),
                    list_order: 2,
                    set_kind: "root",
                    set_group_kind: "root",
                    software_list_name: None,
                    set_name: "later",
                    coverage: "covered",
                },
            )
            .is_err()
        );
        assert!(
            sql_query("UPDATE catalog_covered_sets SET set_name = 'renamed' WHERE coverage_id = ?")
                .bind::<BigInt, _>(id.database_value())
                .execute(connection)
                .is_err()
        );
        assert!(
            sql_query("DELETE FROM catalog_covered_sets WHERE coverage_id = ?")
                .bind::<BigInt, _>(id.database_value())
                .execute(connection)
                .is_err()
        );

        let empty_id = ensure(connection, &CatalogScope::Complete)?;
        connection.batch_execute(&format!(
            "INSERT INTO catalog_snapshots(snapshot_key, coverage_id) VALUES ('empty-snapshot', {})",
            empty_id.database_value()
        ))?;
        assert!(
            sql_query("DELETE FROM catalog_coverage WHERE coverage_id = ?")
                .bind::<BigInt, _>(empty_id.database_value())
                .execute(connection)
                .is_err()
        );
        Ok(())
    })
}

#[derive(diesel::QueryableByName)]
struct MemberCount {
    #[diesel(sql_type = BigInt)]
    member_count: i64,
}
