//! Typed, normalized catalog coverage scopes.
//!
//! Call [`ensure`] inside the caller's SQLite immediate transaction so exact
//! scope lookup and insertion are serialized with publication.

use std::collections::{BTreeMap, BTreeSet};

use diesel::{
    RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};

use crate::domain::{CatalogScope, QualifiedCatalogSet, SetCoverage, SetName};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CoverageId(i64);

impl CoverageId {
    pub(crate) const fn from_database(value: i64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn database_value(self) -> i64 {
        self.0
    }
}

#[derive(diesel::QueryableByName)]
struct CoverageKindRow {
    #[diesel(sql_type = Text)]
    kind: String,
}

#[derive(diesel::QueryableByName)]
struct CoverageIdRow {
    #[diesel(sql_type = BigInt)]
    coverage_id: i64,
}

#[derive(diesel::QueryableByName)]
struct CoveredSetRow {
    #[diesel(sql_type = BigInt)]
    list_order: i64,
    #[diesel(sql_type = Text)]
    set_kind: String,
    #[diesel(sql_type = Text)]
    set_group_kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    software_list_name: Option<String>,
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = Text)]
    coverage: String,
}

/// Return the normalized identity for `scope`, creating its rows if no exact
/// typed scope exists. The caller must hold the immediate transaction that
/// serializes scope deduplication and construction with publication.
pub(super) fn ensure(
    connection: &mut SqliteConnection,
    scope: &CatalogScope,
) -> crate::Result<CoverageId> {
    let existing =
        sql_query("SELECT coverage_id FROM catalog_coverage WHERE kind = ? ORDER BY coverage_id")
            .bind::<Text, _>(scope.kind())
            .load::<CoverageIdRow>(connection)?;

    for row in existing {
        let id = CoverageId::from_database(row.coverage_id);
        if load(connection, id)? == *scope {
            return Ok(id);
        }
    }

    sql_query("INSERT INTO catalog_coverage (kind) VALUES (?)")
        .bind::<Text, _>(scope.kind())
        .execute(connection)?;
    let id = sql_query("SELECT last_insert_rowid() AS coverage_id")
        .get_result::<CoverageIdRow>(connection)?
        .coverage_id;
    let id = CoverageId::from_database(id);

    match scope {
        CatalogScope::Unknown | CatalogScope::Complete => {}
        CatalogScope::Filtered(members) => {
            for (order, member) in members.iter().enumerate() {
                insert_member(connection, id, order, member, SetCoverage::Covered)?;
            }
        }
        CatalogScope::Partial(members) => {
            for (order, (member, coverage)) in members.iter().enumerate() {
                insert_member(connection, id, order, member, *coverage)?;
            }
        }
    }

    Ok(id)
}

/// Load and validate a persisted scope and its canonical member ordering.
pub fn load(connection: &mut SqliteConnection, id: CoverageId) -> crate::Result<CatalogScope> {
    let kind = sql_query("SELECT kind FROM catalog_coverage WHERE coverage_id = ?")
        .bind::<BigInt, _>(id.database_value())
        .get_result::<CoverageKindRow>(connection)
        .map_err(|error| match error {
            diesel::result::Error::NotFound => crate::Error::DatabaseSchema(format!(
                "catalog coverage {} does not exist",
                id.database_value()
            )),
            other => other.into(),
        })?
        .kind;

    let members = load_members(connection, id)?;
    let scope = scope_from_members(&kind, members, id)?;
    Ok(scope)
}

struct LoadedMembers {
    members: BTreeMap<QualifiedCatalogSet, SetCoverage>,
    ordered_members: Vec<QualifiedCatalogSet>,
}

fn load_members(connection: &mut SqliteConnection, id: CoverageId) -> crate::Result<LoadedMembers> {
    let rows = sql_query(
        "SELECT list_order, set_kind, set_group_kind, software_list_name, set_name, coverage \
         FROM catalog_covered_sets WHERE coverage_id = ? ORDER BY list_order",
    )
    .bind::<BigInt, _>(id.database_value())
    .load::<CoveredSetRow>(connection)?;

    let mut members = BTreeMap::new();
    let mut ordered_members = Vec::with_capacity(rows.len());
    for (expected_order, row) in rows.into_iter().enumerate() {
        let expected_order = i64::try_from(expected_order).map_err(|_| {
            crate::Error::DatabaseSchema("catalog coverage has too many members".to_owned())
        })?;
        if row.list_order != expected_order {
            return Err(crate::Error::DatabaseSchema(format!(
                "catalog coverage {} has noncanonical member ordering",
                id.database_value()
            )));
        }

        let (member, coverage) = decode_member(row, id)?;
        if members.insert(member.clone(), coverage).is_some() {
            return Err(crate::Error::DatabaseSchema(format!(
                "catalog coverage {} repeats a typed set member",
                id.database_value()
            )));
        }
        ordered_members.push(member);
    }

    Ok(LoadedMembers {
        members,
        ordered_members,
    })
}

fn decode_member(
    row: CoveredSetRow,
    id: CoverageId,
) -> crate::Result<(QualifiedCatalogSet, SetCoverage)> {
    let member = match (
        row.set_kind.as_str(),
        row.set_group_kind.as_str(),
        row.software_list_name,
    ) {
        ("root", "root", None) => QualifiedCatalogSet::RootSet(SetName::new(row.set_name)),
        ("software_item", "software_list", Some(list_name)) => QualifiedCatalogSet::SoftwareItem {
            list_name,
            name: SetName::new(row.set_name),
        },
        _ => {
            return Err(crate::Error::DatabaseSchema(format!(
                "catalog coverage {} has an invalid typed set qualification",
                id.database_value()
            )));
        }
    };
    let coverage = match row.coverage.as_str() {
        "covered" => SetCoverage::Covered,
        "unknown" => SetCoverage::Unknown,
        _ => {
            return Err(crate::Error::DatabaseSchema(format!(
                "catalog coverage {} has an invalid member coverage value",
                id.database_value()
            )));
        }
    };
    Ok((member, coverage))
}

fn scope_from_members(
    kind: &str,
    loaded: LoadedMembers,
    id: CoverageId,
) -> crate::Result<CatalogScope> {
    let LoadedMembers {
        members,
        ordered_members,
    } = loaded;
    let scope = match kind {
        "unknown" if members.is_empty() => CatalogScope::Unknown,
        "complete" if members.is_empty() => CatalogScope::Complete,
        "unknown" | "complete" => {
            return Err(crate::Error::DatabaseSchema(format!(
                "{} catalog coverage {} cannot have member rows",
                kind,
                id.database_value()
            )));
        }
        "filtered" => {
            let mut filtered = BTreeSet::new();
            for (member, coverage) in members {
                if coverage != SetCoverage::Covered || !filtered.insert(member) {
                    return Err(crate::Error::DatabaseSchema(format!(
                        "filtered catalog coverage {} must contain distinct covered members",
                        id.database_value()
                    )));
                }
            }
            CatalogScope::Filtered(filtered)
        }
        "partial" => CatalogScope::Partial(members),
        _ => {
            return Err(crate::Error::DatabaseSchema(format!(
                "catalog coverage {} has an invalid kind",
                id.database_value()
            )));
        }
    };
    validate_canonical_order(&scope, &ordered_members, id)?;
    Ok(scope)
}

fn validate_canonical_order(
    scope: &CatalogScope,
    ordered_members: &[QualifiedCatalogSet],
    id: CoverageId,
) -> crate::Result<()> {
    let canonical_order = match scope {
        CatalogScope::Unknown | CatalogScope::Complete => ordered_members.is_empty(),
        CatalogScope::Filtered(members) => ordered_members.iter().eq(members.iter()),
        CatalogScope::Partial(members) => ordered_members.iter().eq(members.keys()),
    };
    if !canonical_order {
        return Err(crate::Error::DatabaseSchema(format!(
            "catalog coverage {} has noncanonical member ordering",
            id.database_value()
        )));
    }
    Ok(())
}

fn insert_member(
    connection: &mut SqliteConnection,
    id: CoverageId,
    order: usize,
    member: &QualifiedCatalogSet,
    coverage: SetCoverage,
) -> crate::Result<()> {
    let (set_kind, set_group_kind, list_name, set_name) = match member {
        QualifiedCatalogSet::RootSet(name) => ("root", "root", None, name.as_str()),
        QualifiedCatalogSet::SoftwareItem { list_name, name } => (
            "software_item",
            "software_list",
            Some(list_name.as_str()),
            name.as_str(),
        ),
    };
    let order = i64::try_from(order).map_err(|_| {
        crate::Error::DatabaseSchema("catalog coverage has too many members".to_owned())
    })?;
    sql_query(
        "INSERT INTO catalog_covered_sets \
         (coverage_id, list_order, set_kind, set_group_kind, software_list_name, set_name, coverage) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(id.database_value())
    .bind::<BigInt, _>(order)
    .bind::<Text, _>(set_kind)
    .bind::<Text, _>(set_group_kind)
    .bind::<Nullable<Text>, _>(list_name)
    .bind::<Text, _>(set_name)
    .bind::<Text, _>(match coverage {
        SetCoverage::Covered => "covered",
        SetCoverage::Unknown => "unknown",
    })
    .execute(connection)?;
    Ok(())
}

#[cfg(test)]
mod tests;
