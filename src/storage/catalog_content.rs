use std::collections::{BTreeMap, BTreeSet};

use diesel::{
    SqliteConnection,
    prelude::*,
    sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use crate::domain::CatalogContentId;

#[derive(Clone, Copy)]
pub struct ContentDigestAssertions<'a> {
    scope: &'a str,
    crc32: Option<&'a [u8]>,
    md5: Option<&'a [u8]>,
    sha1: Option<&'a [u8]>,
    sha256: Option<&'a [u8]>,
}

impl<'a> ContentDigestAssertions<'a> {
    #[must_use]
    pub const fn new(
        scope: &'a str,
        crc32: Option<&'a [u8]>,
        md5: Option<&'a [u8]>,
        sha1: Option<&'a [u8]>,
        sha256: Option<&'a [u8]>,
    ) -> Self {
        Self {
            scope,
            crc32,
            md5,
            sha1,
            sha256,
        }
    }

    fn iter(self) -> impl Iterator<Item = DigestAssertion<'a>> {
        [
            self.crc32.map(|value| DigestAssertion {
                algorithm: "crc32",
                scope: self.scope,
                value,
            }),
            self.md5.map(|value| DigestAssertion {
                algorithm: "md5",
                scope: self.scope,
                value,
            }),
            self.sha1.map(|value| DigestAssertion {
                algorithm: "sha1",
                scope: self.scope,
                value,
            }),
            self.sha256.map(|value| DigestAssertion {
                algorithm: "sha256",
                scope: self.scope,
                value,
            }),
        ]
        .into_iter()
        .flatten()
    }
}

#[derive(Clone, Copy)]
pub enum CatalogContentOccurrence<'a> {
    MachineAsset {
        set_id: i64,
        component_order: i64,
    },
    SoftwareComponent {
        snapshot_key: &'a str,
        list_name: &'a str,
        item_name: &'a str,
        part_name: &'a str,
        area_order: i64,
        component_order: i64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentIdentityConflict {
    AmbiguousAlias,
    ContradictoryAssertions,
}

impl ContentIdentityConflict {
    const fn as_str(self) -> &'static str {
        match self {
            Self::AmbiguousAlias => "ambiguous_alias",
            Self::ContradictoryAssertions => "contradictory_assertions",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ContentIdentityResolution {
    Linked(CatalogContentId),
    NoEligibleEvidence,
    Conflict {
        reason: ContentIdentityConflict,
        candidates: Vec<CatalogContentId>,
    },
}

pub fn record_occurrence_digest_assertions(
    connection: &mut SqliteConnection,
    occurrence: CatalogContentOccurrence<'_>,
    assertions: ContentDigestAssertions<'_>,
    provenance: &str,
) -> crate::Result<()> {
    for assertion in assertions.iter() {
        sql_query("INSERT OR IGNORE INTO digest_values (algorithm, digest) VALUES (?, ?)")
            .bind::<Text, _>(assertion.algorithm)
            .bind::<Binary, _>(assertion.value)
            .execute(connection)?;
        let digest_id =
            sql_query("SELECT digest_id FROM digest_values WHERE algorithm = ? AND digest = ?")
                .bind::<Text, _>(assertion.algorithm)
                .bind::<Binary, _>(assertion.value)
                .get_result::<DigestIdRow>(connection)?
                .digest_id;

        match occurrence {
            CatalogContentOccurrence::MachineAsset {
                set_id,
                component_order,
            } => {
                sql_query(
                    "INSERT OR IGNORE INTO asset_requirement_digest_assertions \
                     (set_id, component_order, digest_id, scope, provenance) \
                     VALUES (?, ?, ?, ?, ?)",
                )
                .bind::<BigInt, _>(set_id)
                .bind::<BigInt, _>(component_order)
                .bind::<BigInt, _>(digest_id)
                .bind::<Text, _>(assertion.scope)
                .bind::<Text, _>(provenance)
                .execute(connection)?;
            }
            CatalogContentOccurrence::SoftwareComponent {
                snapshot_key,
                list_name,
                item_name,
                part_name,
                area_order,
                component_order,
            } => {
                sql_query(
                    "INSERT OR IGNORE INTO software_component_digest_assertions \
                     (snapshot_key, list_name, item_name, part_name, area_order, component_order, \
                      digest_id, scope, provenance) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind::<Text, _>(snapshot_key)
                .bind::<Text, _>(list_name)
                .bind::<Text, _>(item_name)
                .bind::<Text, _>(part_name)
                .bind::<BigInt, _>(area_order)
                .bind::<BigInt, _>(component_order)
                .bind::<BigInt, _>(digest_id)
                .bind::<Text, _>(assertion.scope)
                .bind::<Text, _>(provenance)
                .execute(connection)?;
            }
        }
    }
    Ok(())
}

impl ContentIdentityResolution {
    #[must_use]
    pub const fn content_id(&self) -> Option<CatalogContentId> {
        match self {
            Self::Linked(id) => Some(*id),
            Self::NoEligibleEvidence | Self::Conflict { .. } => None,
        }
    }
}

#[derive(Clone, Copy)]
struct DigestAssertion<'a> {
    algorithm: &'static str,
    scope: &'a str,
    value: &'a [u8],
}

#[derive(diesel::QueryableByName)]
struct ContentUuidRow {
    #[diesel(sql_type = Binary)]
    content_uuid: Vec<u8>,
}

#[derive(diesel::QueryableByName)]
struct ContentFactsRow {
    #[diesel(sql_type = Nullable<BigInt>)]
    expected_size: Option<i64>,
}

#[derive(diesel::QueryableByName)]
struct DigestIdRow {
    #[diesel(sql_type = BigInt)]
    digest_id: i64,
}

#[derive(diesel::QueryableByName)]
struct DigestFactRow {
    #[diesel(sql_type = Text)]
    algorithm: String,
    #[diesel(sql_type = Text)]
    scope: String,
    #[diesel(sql_type = Binary)]
    digest: Vec<u8>,
}

pub fn resolve_content_identity(
    connection: &mut SqliteConnection,
    size: Option<i64>,
    assertions: ContentDigestAssertions<'_>,
) -> crate::Result<ContentIdentityResolution> {
    let mut candidates = BTreeSet::new();
    for assertion in assertions
        .iter()
        .filter(|assertion| assertion.identifies_whole_file())
    {
        let rows = sql_query(
            "SELECT assertion.content_uuid FROM catalog_content_digest_assertions AS assertion \
             JOIN digest_values AS digest USING (digest_id) \
             WHERE digest.algorithm = ? AND assertion.scope = ? AND digest.digest = ?",
        )
        .bind::<Text, _>(assertion.algorithm)
        .bind::<Text, _>(assertion.scope)
        .bind::<Binary, _>(assertion.value)
        .load::<ContentUuidRow>(connection)?;

        for row in rows {
            candidates.insert(content_id(row.content_uuid)?);
        }
    }

    let eligible = assertions
        .iter()
        .any(DigestAssertion::identifies_whole_file);
    let (content_id, created) = match candidates.iter().next().copied() {
        None if eligible => {
            let id = CatalogContentId::generate();
            sql_query("INSERT INTO catalog_contents (content_uuid, expected_size) VALUES (?, ?)")
                .bind::<Binary, _>(id.as_bytes().as_slice())
                .bind::<Nullable<BigInt>, _>(size)
                .execute(connection)?;
            (id, true)
        }
        None => return Ok(ContentIdentityResolution::NoEligibleEvidence),
        Some(id) if candidates.len() == 1 => (id, false),
        Some(_) => {
            return Ok(ContentIdentityResolution::Conflict {
                reason: ContentIdentityConflict::AmbiguousAlias,
                candidates: candidates.into_iter().collect(),
            });
        }
    };

    if !compatible_identity(connection, content_id, size, assertions)? {
        if created {
            sql_query("DELETE FROM catalog_contents WHERE content_uuid = ?")
                .bind::<Binary, _>(content_id.as_bytes().as_slice())
                .execute(connection)?;
        }
        return Ok(ContentIdentityResolution::Conflict {
            reason: ContentIdentityConflict::ContradictoryAssertions,
            candidates: vec![content_id],
        });
    }

    sql_query(
        "UPDATE catalog_contents SET expected_size = COALESCE(expected_size, ?) \
         WHERE content_uuid = ?",
    )
    .bind::<Nullable<BigInt>, _>(size)
    .bind::<Binary, _>(content_id.as_bytes().as_slice())
    .execute(connection)?;

    for assertion in assertions.iter() {
        sql_query("INSERT OR IGNORE INTO digest_values (algorithm, digest) VALUES (?, ?)")
            .bind::<Text, _>(assertion.algorithm)
            .bind::<Binary, _>(assertion.value)
            .execute(connection)?;
        let digest_id =
            sql_query("SELECT digest_id FROM digest_values WHERE algorithm = ? AND digest = ?")
                .bind::<Text, _>(assertion.algorithm)
                .bind::<Binary, _>(assertion.value)
                .get_result::<DigestIdRow>(connection)?
                .digest_id;
        sql_query(
            "INSERT OR IGNORE INTO catalog_content_digest_assertions \
             (content_uuid, digest_id, scope) VALUES (?, ?, ?)",
        )
        .bind::<Binary, _>(content_id.as_bytes().as_slice())
        .bind::<BigInt, _>(digest_id)
        .bind::<Text, _>(assertion.scope)
        .execute(connection)?;
    }

    Ok(ContentIdentityResolution::Linked(content_id))
}

pub fn record_content_identity_conflict(
    connection: &mut SqliteConnection,
    occurrence: CatalogContentOccurrence<'_>,
    resolution: &ContentIdentityResolution,
) -> crate::Result<()> {
    let ContentIdentityResolution::Conflict { reason, candidates } = resolution else {
        return Ok(());
    };
    for candidate in candidates {
        match occurrence {
            CatalogContentOccurrence::MachineAsset {
                set_id,
                component_order,
            } => {
                sql_query(
                    "INSERT OR IGNORE INTO asset_requirement_content_conflicts \
                     (set_id, component_order, candidate_content_uuid, reason) VALUES (?, ?, ?, ?)",
                )
                .bind::<BigInt, _>(set_id)
                .bind::<BigInt, _>(component_order)
                .bind::<Binary, _>(candidate.as_bytes().as_slice())
                .bind::<Text, _>(reason.as_str())
                .execute(connection)?;
            }
            CatalogContentOccurrence::SoftwareComponent {
                snapshot_key,
                list_name,
                item_name,
                part_name,
                area_order,
                component_order,
            } => {
                sql_query(
                    "INSERT OR IGNORE INTO software_component_content_conflicts \
                     (snapshot_key, list_name, item_name, part_name, area_order, component_order, \
                      candidate_content_uuid, reason) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind::<Text, _>(snapshot_key)
                .bind::<Text, _>(list_name)
                .bind::<Text, _>(item_name)
                .bind::<Text, _>(part_name)
                .bind::<BigInt, _>(area_order)
                .bind::<BigInt, _>(component_order)
                .bind::<Binary, _>(candidate.as_bytes().as_slice())
                .bind::<Text, _>(reason.as_str())
                .execute(connection)?;
            }
        }
    }
    Ok(())
}

impl DigestAssertion<'_> {
    fn identifies_whole_file(self) -> bool {
        matches!(self.algorithm, "sha1" | "sha256")
            && matches!(self.scope, "whole_asset" | "whole_file")
    }
}

fn compatible_identity(
    connection: &mut SqliteConnection,
    content_id: CatalogContentId,
    size: Option<i64>,
    assertions: ContentDigestAssertions<'_>,
) -> crate::Result<bool> {
    let facts = sql_query("SELECT expected_size FROM catalog_contents WHERE content_uuid = ?")
        .bind::<Binary, _>(content_id.as_bytes().as_slice())
        .get_result::<ContentFactsRow>(connection)?;
    if matches!((facts.expected_size, size), (Some(existing), Some(incoming)) if existing != incoming)
    {
        return Ok(false);
    }

    let stored = sql_query(
        "SELECT digest.algorithm, assertion.scope, digest.digest \
         FROM catalog_content_digest_assertions AS assertion \
         JOIN digest_values AS digest USING (digest_id) \
         WHERE assertion.content_uuid = ?",
    )
    .bind::<Binary, _>(content_id.as_bytes().as_slice())
    .load::<DigestFactRow>(connection)?;
    let mut known = BTreeMap::new();
    for fact in stored {
        let key = (fact.algorithm, fact.scope);
        if known
            .insert(key.clone(), fact.digest.clone())
            .is_some_and(|previous| previous != fact.digest)
        {
            return Ok(false);
        }
    }

    Ok(assertions.iter().all(|assertion| {
        known
            .get(&(assertion.algorithm.to_owned(), assertion.scope.to_owned()))
            .is_none_or(|value| value.as_slice() == assertion.value)
    }))
}

fn content_id(bytes: Vec<u8>) -> crate::Result<CatalogContentId> {
    let bytes: [u8; 16] = bytes.try_into().map_err(|bytes: Vec<u8>| {
        crate::Error::InvalidHash(format!(
            "catalog content UUID BLOB contains {} bytes; expected 16",
            bytes.len()
        ))
    })?;
    Ok(CatalogContentId::from_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use diesel::{Connection, connection::SimpleConnection};

    use super::*;

    #[test]
    fn sha1_sha256_bridge_stays_ambiguous_in_either_order() -> crate::Result<()> {
        for sha256_first in [false, true] {
            let mut connection = registry_connection()?;
            let sha1 = [0x11; 20];
            let sha256 = [0x22; 32];
            let first = if sha256_first {
                ContentDigestAssertions::new("whole_file", None, None, None, Some(&sha256))
            } else {
                ContentDigestAssertions::new("whole_file", None, None, Some(&sha1), None)
            };
            let second = if sha256_first {
                ContentDigestAssertions::new("whole_file", None, None, Some(&sha1), None)
            } else {
                ContentDigestAssertions::new("whole_file", None, None, None, Some(&sha256))
            };

            assert!(matches!(
                resolve_content_identity(&mut connection, None, first)?,
                ContentIdentityResolution::Linked(_)
            ));
            assert!(matches!(
                resolve_content_identity(&mut connection, None, second)?,
                ContentIdentityResolution::Linked(_)
            ));
            let bridge = resolve_content_identity(
                &mut connection,
                None,
                ContentDigestAssertions::new("whole_file", None, None, Some(&sha1), Some(&sha256)),
            )?;
            match bridge {
                ContentIdentityResolution::Conflict { candidates, .. } => {
                    assert_eq!(candidates.len(), 2);
                }
                _ => {
                    return Err(crate::Error::InvalidPath(
                        "dual-digest bridge unexpectedly resolved".to_owned(),
                    ));
                }
            }
            let identities = sql_query("SELECT COUNT(*) AS count FROM catalog_contents")
                .get_result::<CountRow>(&mut connection)?;
            assert_eq!(identities.count, 2);
        }
        Ok(())
    }

    #[derive(diesel::QueryableByName)]
    struct CountRow {
        #[diesel(sql_type = BigInt)]
        count: i64,
    }

    fn registry_connection() -> crate::Result<SqliteConnection> {
        let mut connection = SqliteConnection::establish(":memory:")
            .map_err(|error| crate::Error::InvalidPath(error.to_string()))?;
        connection.batch_execute(
            "CREATE TABLE catalog_contents (
                 content_uuid BLOB PRIMARY KEY NOT NULL CHECK (length(content_uuid) = 16),
                 expected_size INTEGER
             );
             CREATE TABLE digest_values (
                 digest_id INTEGER PRIMARY KEY,
                 algorithm TEXT NOT NULL,
                 digest BLOB NOT NULL,
                 UNIQUE (algorithm, digest)
             );
             CREATE TABLE catalog_content_digest_assertions (
                 content_uuid BLOB NOT NULL,
                 digest_id INTEGER NOT NULL,
                 scope TEXT NOT NULL,
                 PRIMARY KEY (content_uuid, digest_id, scope),
                 FOREIGN KEY (content_uuid) REFERENCES catalog_contents (content_uuid),
                 FOREIGN KEY (digest_id) REFERENCES digest_values (digest_id)
             ) WITHOUT ROWID;",
        )?;
        Ok(connection)
    }
}
