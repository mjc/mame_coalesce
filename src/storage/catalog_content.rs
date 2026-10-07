use std::collections::{BTreeMap, BTreeSet};

use diesel::{
    RunQueryDsl, SqliteConnection,
    prelude::*,
    query_builder::{AstPass, Query, QueryFragment, QueryId},
    query_dsl::methods::ExecuteDsl,
    sql_query,
    sql_types::{BigInt, Binary, Text},
    sqlite::Sqlite,
};

use super::catalog_identity::OccurrenceId;
use crate::domain::{CatalogContentId, CatalogRegistryId, ContentDigestAlgorithm};

/// Seed reverse traversal with one canonical UUID; unrelated files are never visited.
pub const FILE_COMPONENT_SQL: &str = "WITH RECURSIVE component(content_uuid) AS ( \
    SELECT ? UNION SELECT redirect.old_content_uuid FROM component \
    JOIN merged_file_ids AS redirect ON redirect.kept_content_uuid = component.content_uuid \
    JOIN file_match_decision_publications USING (decision_id)) ";

#[derive(diesel::QueryableByName)]
struct RegistryRow {
    #[diesel(sql_type = Binary)]
    registry_uuid: Vec<u8>,
}

/// Identity-registry generation preserved by paired database backups.
///
/// # Errors
/// Rejects missing generations and values that are not sixteen-byte SQLite BLOBs.
pub fn registry_id(connection: &mut SqliteConnection) -> crate::Result<CatalogRegistryId> {
    let row = sql_query(
        "SELECT registry_uuid FROM file_id_registries WHERE registry_id = 1
         AND typeof(registry_uuid) = 'blob' AND length(registry_uuid) = 16",
    )
    .get_result::<RegistryRow>(connection)
    .map_err(|error| {
        crate::Error::DatabaseSchema(format!(
            "missing or invalid identity registry generation: {error}"
        ))
    })?;
    let bytes = row.registry_uuid.try_into().map_err(|bytes: Vec<u8>| {
        crate::Error::DatabaseSchema(format!(
            "registry UUID contains {} bytes; expected 16",
            bytes.len()
        ))
    })?;
    Ok(CatalogRegistryId::from_bytes(bytes))
}

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
                algorithm: ContentDigestAlgorithm::Crc32,
                scope: self.scope,
                value,
            }),
            self.md5.map(|value| DigestAssertion {
                algorithm: ContentDigestAlgorithm::Md5,
                scope: self.scope,
                value,
            }),
            self.sha1.map(|value| DigestAssertion {
                algorithm: ContentDigestAlgorithm::Sha1,
                scope: self.scope,
                value,
            }),
            self.sha256.map(|value| DigestAssertion {
                algorithm: ContentDigestAlgorithm::Sha256,
                scope: self.scope,
                value,
            }),
        ]
        .into_iter()
        .flatten()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentIdentityConflict {
    AmbiguousAlias,
    ContradictoryAssertions,
    DisputedAlias,
}

impl ContentIdentityConflict {
    const fn as_str(self) -> &'static str {
        match self {
            Self::AmbiguousAlias => "ambiguous_alias",
            Self::ContradictoryAssertions => "contradictory_assertions",
            Self::DisputedAlias => "disputed_alias",
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

struct DigestValueInsertQuery;

struct DigestValueInsert<'a> {
    algorithm: &'a str,
    bytes: &'a [u8],
}

impl QueryId for DigestValueInsert<'_> {
    type QueryId = DigestValueInsertQuery;
}

impl QueryFragment<Sqlite> for DigestValueInsert<'_> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql("INSERT OR IGNORE INTO digest_values (algorithm, digest) VALUES (");
        pass.push_bind_param::<Text, _>(&self.algorithm)?;
        pass.push_sql(", ");
        pass.push_bind_param::<Binary, _>(&self.bytes)?;
        pass.push_sql(")");
        Ok(())
    }
}

impl RunQueryDsl<SqliteConnection> for DigestValueInsert<'_> {}

struct DigestValueSelectQuery;

struct DigestValueSelect<'a> {
    algorithm: &'a str,
    bytes: &'a [u8],
}

impl QueryId for DigestValueSelect<'_> {
    type QueryId = DigestValueSelectQuery;
}

impl Query for DigestValueSelect<'_> {
    type SqlType = BigInt;
}

impl QueryFragment<Sqlite> for DigestValueSelect<'_> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql("SELECT digest_id FROM digest_values WHERE algorithm = ");
        pass.push_bind_param::<Text, _>(&self.algorithm)?;
        pass.push_sql(" AND digest = ");
        pass.push_bind_param::<Binary, _>(&self.bytes)?;
        Ok(())
    }
}

impl RunQueryDsl<SqliteConnection> for DigestValueSelect<'_> {}

struct OccurrenceDigestAssertionInsertQuery;

struct OccurrenceDigestAssertionInsert<'a> {
    occurrence_id: i64,
    digest_id: i64,
    scope: &'a str,
    provenance: &'a str,
}

impl QueryId for OccurrenceDigestAssertionInsert<'_> {
    type QueryId = OccurrenceDigestAssertionInsertQuery;
}

impl QueryFragment<Sqlite> for OccurrenceDigestAssertionInsert<'_> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql(
            "INSERT OR IGNORE INTO occurrence_digest_assertions \
             (occurrence_id, digest_id, scope, provenance) VALUES (",
        );
        pass.push_bind_param::<BigInt, _>(&self.occurrence_id)?;
        pass.push_sql(", ");
        pass.push_bind_param::<BigInt, _>(&self.digest_id)?;
        pass.push_sql(", ");
        pass.push_bind_param::<Text, _>(&self.scope)?;
        pass.push_sql(", ");
        pass.push_bind_param::<Text, _>(&self.provenance)?;
        pass.push_sql(")");
        Ok(())
    }
}

impl RunQueryDsl<SqliteConnection> for OccurrenceDigestAssertionInsert<'_> {}

pub fn record_occurrence_digest_assertions(
    connection: &mut SqliteConnection,
    occurrence: OccurrenceId,
    assertions: ContentDigestAssertions<'_>,
    provenance: &str,
) -> crate::Result<()> {
    for assertion in assertions.iter() {
        let algorithm = assertion.algorithm.as_str();
        let digest_id = intern_digest(connection, algorithm, assertion.value)?;
        ExecuteDsl::execute(
            OccurrenceDigestAssertionInsert {
                occurrence_id: occurrence.database_value(),
                digest_id,
                scope: assertion.scope,
                provenance,
            },
            connection,
        )?;
    }
    Ok(())
}

/// Reuse the binary digest value without inventing evidence scope or ownership.
pub fn intern_digest(
    connection: &mut SqliteConnection,
    algorithm: &str,
    bytes: &[u8],
) -> crate::Result<i64> {
    ExecuteDsl::execute(DigestValueInsert { algorithm, bytes }, connection)?;
    Ok(DigestValueSelect { algorithm, bytes }.get_result::<i64>(connection)?)
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
    algorithm: ContentDigestAlgorithm,
    scope: &'a str,
    value: &'a [u8],
}

#[derive(diesel::QueryableByName)]
struct ContentUuidRow {
    #[diesel(sql_type = Binary)]
    content_uuid: Vec<u8>,
}

struct ContentIdentityLookupQuery;

struct ContentIdentityLookup<'a> {
    algorithm: &'a str,
    digest: &'a [u8],
}

impl QueryId for ContentIdentityLookup<'_> {
    type QueryId = ContentIdentityLookupQuery;
}

impl Query for ContentIdentityLookup<'_> {
    type SqlType = (Binary, BigInt);
}

impl RunQueryDsl<SqliteConnection> for ContentIdentityLookup<'_> {}

impl QueryFragment<Sqlite> for ContentIdentityLookup<'_> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql(
            "SELECT DISTINCT assertion.content_uuid AS content_uuid, 0 AS disputed \
             FROM catalog_content_digest_assertions AS assertion \
             JOIN digest_values AS digest USING (digest_id) \
             WHERE digest.algorithm = ",
        );
        pass.push_bind_param::<Text, _>(&self.algorithm)?;
        pass.push_sql(" AND digest.digest = ");
        pass.push_bind_param::<Binary, _>(&self.digest)?;
        pass.push_sql(
            " UNION ALL \
             SELECT DISTINCT dispute.candidate_content_uuid AS content_uuid, 1 AS disputed \
             FROM disputed_file_hashes AS dispute \
             JOIN digest_values AS digest USING (digest_id) \
             WHERE digest.algorithm = ",
        );
        pass.push_bind_param::<Text, _>(&self.algorithm)?;
        pass.push_sql(" AND digest.digest = ");
        pass.push_bind_param::<Binary, _>(&self.digest)?;
        Ok(())
    }
}

struct CatalogContentInsertQuery;

struct CatalogContentInsert(CatalogContentId);

impl QueryId for CatalogContentInsert {
    type QueryId = CatalogContentInsertQuery;
}

impl RunQueryDsl<SqliteConnection> for CatalogContentInsert {}

impl QueryFragment<Sqlite> for CatalogContentInsert {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql("INSERT INTO catalog_contents (content_uuid) VALUES (");
        let bytes = self.0.as_bytes();
        pass.push_bind_param::<Binary, _>(bytes)?;
        pass.push_sql(")");
        Ok(())
    }
}

struct IssuedContentResolutionQuery;

struct IssuedContentResolution<'a>(&'a [u8]);

impl QueryId for IssuedContentResolution<'_> {
    type QueryId = IssuedContentResolutionQuery;
}

impl Query for IssuedContentResolution<'_> {
    type SqlType = Binary;
}

impl RunQueryDsl<SqliteConnection> for IssuedContentResolution<'_> {}

impl QueryFragment<Sqlite> for IssuedContentResolution<'_> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql(
            "WITH RECURSIVE path(content_uuid) AS ( \
             SELECT content_uuid FROM catalog_contents WHERE content_uuid = ",
        );
        pass.push_bind_param::<Binary, _>(&self.0)?;
        pass.push_sql(
            " UNION SELECT redirect.kept_content_uuid FROM path \
             JOIN merged_file_ids AS redirect ON redirect.old_content_uuid = path.content_uuid \
             JOIN file_match_decision_publications USING (decision_id)) \
             SELECT content_uuid FROM path WHERE NOT EXISTS ( \
             SELECT 1 FROM merged_file_ids AS redirect \
             JOIN file_match_decision_publications USING (decision_id) \
             WHERE redirect.old_content_uuid = path.content_uuid)",
        );
        Ok(())
    }
}

struct ContentIdentitySizesQuery;

struct ContentIdentitySizes<'a>(&'a [u8]);

impl QueryId for ContentIdentitySizes<'_> {
    type QueryId = ContentIdentitySizesQuery;
}

impl Query for ContentIdentitySizes<'_> {
    type SqlType = BigInt;
}

impl RunQueryDsl<SqliteConnection> for ContentIdentitySizes<'_> {}

impl QueryFragment<Sqlite> for ContentIdentitySizes<'_> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql("WITH RECURSIVE component(content_uuid) AS ( SELECT ");
        pass.push_bind_param::<Binary, _>(&self.0)?;
        pass.push_sql(
            " UNION SELECT redirect.old_content_uuid FROM component \
             JOIN merged_file_ids AS redirect ON redirect.kept_content_uuid = component.content_uuid \
             JOIN file_match_decision_publications USING (decision_id)) \
             SELECT DISTINCT (SELECT sizes.size FROM accepted_file_size_assertions AS sizes \
             WHERE sizes.occurrence_id = entry.occurrence_id) AS size FROM component \
             CROSS JOIN asset_occurrences AS entry ON entry.content_uuid = component.content_uuid \
             WHERE size IS NOT NULL LIMIT 2",
        );
        Ok(())
    }
}

struct ContentIdentityDigestsQuery;

struct ContentIdentityDigests<'a>(&'a [u8]);

impl QueryId for ContentIdentityDigests<'_> {
    type QueryId = ContentIdentityDigestsQuery;
}

impl Query for ContentIdentityDigests<'_> {
    type SqlType = (Text, Binary);
}

impl RunQueryDsl<SqliteConnection> for ContentIdentityDigests<'_> {}

impl QueryFragment<Sqlite> for ContentIdentityDigests<'_> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql("WITH RECURSIVE component(content_uuid) AS ( SELECT ");
        pass.push_bind_param::<Binary, _>(&self.0)?;
        pass.push_sql(
            " UNION SELECT redirect.old_content_uuid FROM component \
             JOIN merged_file_ids AS redirect ON redirect.kept_content_uuid = component.content_uuid \
             JOIN file_match_decision_publications USING (decision_id)) \
             SELECT DISTINCT digest.algorithm, digest.digest FROM component \
             CROSS JOIN asset_occurrences AS entry ON entry.content_uuid = component.content_uuid \
             CROSS JOIN catalog_content_digest_assertions AS assertion \
                 ON assertion.occurrence_id = entry.occurrence_id \
             CROSS JOIN digest_values AS digest ON digest.digest_id = assertion.digest_id",
        );
        Ok(())
    }
}

/// Follow published redirects from one issued UUID using primary-key seeks.
pub fn resolve_issued_id(
    connection: &mut SqliteConnection,
    issued: CatalogContentId,
) -> crate::Result<CatalogContentId> {
    let rows = IssuedContentResolution(issued.as_bytes()).load::<Vec<u8>>(connection)?;
    match rows.as_slice() {
        [row] => content_id(row.clone()),
        [] => {
            let known =
                sql_query("SELECT content_uuid FROM catalog_contents WHERE content_uuid = ?")
                    .bind::<Binary, _>(issued.as_bytes().as_slice())
                    .get_result::<ContentUuidRow>(connection)
                    .optional()?
                    .map(|row| row.content_uuid);
            Err(if known.is_some() {
                super::file_match_reviews::ReviewError::CorruptRedirects
            } else {
                super::file_match_reviews::ReviewError::UnknownFile(issued)
            }
            .into())
        }
        _ => Err(super::file_match_reviews::ReviewError::CorruptRedirects.into()),
    }
}

#[cfg(test)]
#[derive(diesel::QueryableByName)]
struct ContentFactsRow {
    #[diesel(sql_type = BigInt)]
    size: i64,
}

/// Resolve source assertions without storing another copy of their hashes or size.
/// The caller persists the linked native occurrence and its assertions in the
/// same import transaction before resolving the next occurrence.
pub fn resolve_content_identity(
    connection: &mut SqliteConnection,
    size: Option<i64>,
    assertions: ContentDigestAssertions<'_>,
) -> crate::Result<ContentIdentityResolution> {
    if size.is_some_and(|size| size < 0) {
        return Err(crate::Error::InvalidPath(
            "whole-file size cannot be negative".to_owned(),
        ));
    }
    for assertion in assertions.iter() {
        let expected = assertion.algorithm.byte_length();
        if assertion.value.len() != expected {
            return Err(crate::Error::InvalidHash(format!(
                "{} assertion contains {} bytes; expected {expected}",
                assertion.algorithm.as_str(),
                assertion.value.len()
            )));
        }
    }
    let eligible = assertions
        .iter()
        .any(DigestAssertion::identifies_whole_file);
    if !eligible {
        return Ok(ContentIdentityResolution::NoEligibleEvidence);
    }

    let mut candidates = BTreeSet::new();
    let mut disputed = false;
    for assertion in assertions
        .iter()
        .filter(|assertion| assertion.identifies_whole_file())
    {
        let rows = ContentIdentityLookup {
            algorithm: assertion.algorithm.as_str(),
            digest: assertion.value,
        }
        .load::<(Vec<u8>, i64)>(connection)?;
        for (candidate, is_disputed) in rows {
            disputed |= is_disputed != 0;
            candidates.insert(resolve_issued_id(connection, content_id(candidate)?)?);
        }
    }
    if disputed {
        return Ok(ContentIdentityResolution::Conflict {
            reason: ContentIdentityConflict::DisputedAlias,
            candidates: candidates.into_iter().collect(),
        });
    }
    match candidates.len() {
        0 => {
            let id = CatalogContentId::generate();
            ExecuteDsl::execute(CatalogContentInsert(id), connection)?;
            Ok(ContentIdentityResolution::Linked(id))
        }
        1 => {
            let Some(id) = candidates.into_iter().next() else {
                return Err(crate::Error::InvalidPath(
                    "missing unique content candidate".to_owned(),
                ));
            };
            if compatible_identity(connection, id, size, assertions)? {
                Ok(ContentIdentityResolution::Linked(id))
            } else {
                Ok(ContentIdentityResolution::Conflict {
                    reason: ContentIdentityConflict::ContradictoryAssertions,
                    candidates: vec![id],
                })
            }
        }
        _ => Ok(ContentIdentityResolution::Conflict {
            reason: ContentIdentityConflict::AmbiguousAlias,
            candidates: candidates.into_iter().collect(),
        }),
    }
}

pub fn record_content_identity_conflict(
    connection: &mut SqliteConnection,
    occurrence: OccurrenceId,
    resolution: &ContentIdentityResolution,
) -> crate::Result<()> {
    let ContentIdentityResolution::Conflict { reason, candidates } = resolution else {
        return Ok(());
    };
    for candidate in candidates {
        sql_query(
            "INSERT OR IGNORE INTO occurrence_content_conflicts \
             (occurrence_id, candidate_content_uuid, reason) VALUES (?, ?, ?)",
        )
        .bind::<BigInt, _>(occurrence.database_value())
        .bind::<Binary, _>(candidate.as_bytes().as_slice())
        .bind::<Text, _>(reason.as_str())
        .execute(connection)?;
        sql_query(
            "INSERT INTO occurrence_content_conflict_hashes \
             (occurrence_id, candidate_content_uuid, evidence_occurrence_id, digest_id, scope, provenance, role) \
             SELECT ?, ?, occurrence_id, digest_id, scope, provenance, 'incoming' \
             FROM occurrence_digest_assertions WHERE occurrence_id = ? \
             AND scope IN ('whole_asset', 'whole_file') AND provenance = 'source_declared'",
        )
        .bind::<BigInt, _>(occurrence.database_value())
        .bind::<Binary, _>(candidate.as_bytes().as_slice())
        .bind::<BigInt, _>(occurrence.database_value())
        .execute(connection)?;
        sql_query(format!(
            "{FILE_COMPONENT_SQL} INSERT INTO occurrence_content_conflict_hashes \
             (occurrence_id, candidate_content_uuid, evidence_occurrence_id, digest_id, scope, provenance, role) \
             SELECT ?, ?, assertion.occurrence_id, assertion.digest_id, assertion.scope, assertion.provenance, 'candidate' \
             FROM component CROSS JOIN asset_occurrences AS entry ON entry.content_uuid = component.content_uuid \
             CROSS JOIN catalog_content_digest_assertions AS assertion ON assertion.occurrence_id = entry.occurrence_id",
        ))
        .bind::<Binary, _>(candidate.as_bytes().as_slice())
        .bind::<BigInt, _>(occurrence.database_value())
        .bind::<Binary, _>(candidate.as_bytes().as_slice())
        .execute(connection)?;
        sql_query(
            "INSERT INTO occurrence_content_conflict_sizes \
             (occurrence_id, candidate_content_uuid, evidence_occurrence_id, size_field, role) \
             SELECT ?, ?, occurrence_id, size_field, 'incoming' \
             FROM accepted_file_size_assertions WHERE occurrence_id = ?",
        )
        .bind::<BigInt, _>(occurrence.database_value())
        .bind::<Binary, _>(candidate.as_bytes().as_slice())
        .bind::<BigInt, _>(occurrence.database_value())
        .execute(connection)?;
        sql_query(candidate_size_evidence_insert())
            .bind::<Binary, _>(candidate.as_bytes().as_slice())
            .bind::<BigInt, _>(occurrence.database_value())
            .bind::<Binary, _>(candidate.as_bytes().as_slice())
            .execute(connection)?;
    }
    Ok(())
}

impl DigestAssertion<'_> {
    fn identifies_whole_file(self) -> bool {
        matches!(
            self.algorithm,
            ContentDigestAlgorithm::Sha1 | ContentDigestAlgorithm::Sha256
        ) && matches!(self.scope, "whole_asset" | "whole_file")
    }
}

fn compatible_identity(
    connection: &mut SqliteConnection,
    content_id: CatalogContentId,
    size: Option<i64>,
    assertions: ContentDigestAssertions<'_>,
) -> crate::Result<bool> {
    let facts = ContentIdentitySizes(content_id.as_bytes()).load::<i64>(connection)?;
    if facts.len() > 1
        || size.is_some_and(|incoming| facts.first().is_some_and(|fact| *fact != incoming))
    {
        return Ok(false);
    }

    let stored =
        ContentIdentityDigests(content_id.as_bytes()).load::<(String, Vec<u8>)>(connection)?;
    let mut known = BTreeMap::new();
    for (algorithm, digest) in stored {
        match known.entry(algorithm) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(digest);
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                if entry.get() != &digest {
                    return Ok(false);
                }
            }
        }
    }

    Ok(assertions.iter().all(|assertion| {
        known
            .get(assertion.algorithm.as_str())
            .is_none_or(|value| value.as_slice() == assertion.value)
    }))
}

#[cfg(test)]
fn identity_size_select() -> String {
    format!(
        "{FILE_COMPONENT_SQL} SELECT DISTINCT \
         (SELECT sizes.size FROM accepted_file_size_assertions AS sizes \
          WHERE sizes.occurrence_id = entry.occurrence_id) AS size FROM component \
         CROSS JOIN asset_occurrences AS entry ON entry.content_uuid = component.content_uuid \
         WHERE size IS NOT NULL \
         LIMIT 2"
    )
}

fn candidate_size_evidence_insert() -> String {
    format!(
        "{FILE_COMPONENT_SQL} INSERT INTO occurrence_content_conflict_sizes \
         (occurrence_id, candidate_content_uuid, evidence_occurrence_id, size_field, role) \
         SELECT ?, ?, sizes.occurrence_id, sizes.size_field, 'candidate' \
         FROM component CROSS JOIN asset_occurrences AS entry ON entry.content_uuid = component.content_uuid \
         CROSS JOIN accepted_file_size_assertions AS sizes ON sizes.occurrence_id = entry.occurrence_id"
    )
}

#[cfg(test)]
mod size_tests;

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
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use diesel::{
        Connection,
        connection::{InstrumentationEvent, SimpleConnection},
    };

    use super::*;

    #[test]
    fn equivalent_whole_file_scopes_share_identity() -> crate::Result<()> {
        let mut connection = registry_connection()?;
        let sha1 = [0x44; 20];
        let first = resolve_content_identity(
            &mut connection,
            Some(16),
            ContentDigestAssertions::new("whole_file", None, None, Some(&sha1), None),
        )?;
        support_identity(
            &mut connection,
            &first,
            Some(16),
            ContentDigestAssertions::new("whole_file", None, None, Some(&sha1), None),
        )?;
        let second = resolve_content_identity(
            &mut connection,
            Some(16),
            ContentDigestAssertions::new("whole_asset", None, None, Some(&sha1), None),
        )?;
        assert_eq!(first.content_id(), second.content_id());
        Ok(())
    }

    #[test]
    fn digest_assertion_statements_reuse_prepared_queries_for_changed_values() -> crate::Result<()>
    {
        let mut connection = registry_connection()?;
        let occurrences = [1_i64, 2_i64];
        let digest_values = [[0x11; 20], [0x22; 20]];
        for occurrence in occurrences {
            sql_query("INSERT INTO asset_occurrences(occurrence_id) VALUES (?)")
                .bind::<BigInt, _>(occurrence)
                .execute(&mut connection)?;
        }

        let cached_queries = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&cached_queries);
        connection.set_instrumentation(move |event: InstrumentationEvent<'_>| {
            if matches!(event, InstrumentationEvent::CacheQuery { .. }) {
                observed.fetch_add(1, Ordering::Relaxed);
            }
        });

        for (occurrence, digest) in occurrences.into_iter().zip(digest_values) {
            record_occurrence_digest_assertions(
                &mut connection,
                OccurrenceId::from_database(occurrence),
                ContentDigestAssertions::new("whole_file", None, None, Some(&digest), None),
                "source_declared",
            )?;
        }

        let digests = sql_query(
            "SELECT digest_id AS id, digest FROM digest_values WHERE algorithm='sha1' ORDER BY digest",
        )
        .load::<DigestTestRow>(&mut connection)?;
        assert_eq!(digests.len(), 2);
        assert_ne!(digests[0].id, digests[1].id);
        assert_eq!(digests[0].digest.as_slice(), digest_values[0]);
        assert_eq!(digests[1].digest.as_slice(), digest_values[1]);

        let assertion_count =
            sql_query("SELECT COUNT(*) AS count FROM occurrence_digest_assertions")
                .get_result::<CountRow>(&mut connection)?
                .count;
        assert_eq!(assertion_count, 2);
        assert_eq!(cached_queries.load(Ordering::Relaxed), 3);
        Ok(())
    }

    #[test]
    fn content_identity_lookup_reuses_one_prepared_query_for_each_digest() -> crate::Result<()> {
        let mut connection = registry_connection()?;
        let cache_hits = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&cache_hits);
        connection.set_instrumentation(move |event: InstrumentationEvent<'_>| {
            if let InstrumentationEvent::CacheQuery { sql, .. } = event
                && sql.to_string().contains("disputed_file_hashes AS dispute")
            {
                observed.fetch_add(1, Ordering::Relaxed);
            }
        });

        let digest = [0x71; 20];
        let assertions =
            ContentDigestAssertions::new("whole_file", None, None, Some(&digest), None);
        let first = resolve_content_identity(&mut connection, Some(24), assertions)?;
        support_identity(&mut connection, &first, Some(24), assertions)?;
        let second = resolve_content_identity(&mut connection, Some(24), assertions)?;

        assert_eq!(first.content_id(), second.content_id());
        assert_eq!(cache_hits.load(Ordering::Relaxed), 1);
        Ok(())
    }

    #[derive(diesel::QueryableByName)]
    struct DigestTestRow {
        #[diesel(sql_type = BigInt)]
        id: i64,
        #[diesel(sql_type = Binary)]
        digest: Vec<u8>,
    }

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

            let first_id = resolve_content_identity(&mut connection, None, first)?;
            support_identity(&mut connection, &first_id, None, first)?;
            let second_id = resolve_content_identity(&mut connection, None, second)?;
            support_identity(&mut connection, &second_id, None, second)?;
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

    #[test]
    fn unknown_incoming_size_cannot_link_an_inconsistent_candidate() -> crate::Result<()> {
        let mut connection = registry_connection()?;
        let digest = [0x11; 20];
        let assertions =
            ContentDigestAssertions::new("whole_file", None, None, Some(&digest), None);
        let candidate = resolve_content_identity(&mut connection, Some(8), assertions)?;
        support_identity(&mut connection, &candidate, Some(8), assertions)?;
        support_identity(&mut connection, &candidate, Some(4), assertions)?;
        assert!(matches!(
            resolve_content_identity(&mut connection, None, assertions)?,
            ContentIdentityResolution::Conflict {
                reason: ContentIdentityConflict::ContradictoryAssertions,
                ..
            }
        ));
        Ok(())
    }

    fn support_identity(
        connection: &mut SqliteConnection,
        resolution: &ContentIdentityResolution,
        size: Option<i64>,
        assertions: ContentDigestAssertions<'_>,
    ) -> crate::Result<()> {
        #[derive(diesel::QueryableByName)]
        struct Owner {
            #[diesel(sql_type = BigInt)]
            occurrence_id: i64,
        }
        let id = resolution
            .content_id()
            .ok_or_else(|| crate::Error::InvalidPath("test identity missing".to_owned()))?;
        let owner = sql_query(
            "INSERT INTO asset_occurrences(content_uuid) VALUES (?) RETURNING occurrence_id",
        )
        .bind::<Binary, _>(id.as_bytes().as_slice())
        .get_result::<Owner>(connection)?;
        if let Some(size) = size {
            sql_query("INSERT INTO native_file_sizes (occurrence_id,size) VALUES (?,?)")
                .bind::<BigInt, _>(owner.occurrence_id)
                .bind::<BigInt, _>(size)
                .execute(connection)?;
        }
        record_occurrence_digest_assertions(
            connection,
            OccurrenceId::from_database(owner.occurrence_id),
            assertions,
            "source_declared",
        )
    }

    fn registry_connection() -> crate::Result<SqliteConnection> {
        let mut connection = SqliteConnection::establish(":memory:")
            .map_err(|error| crate::Error::InvalidPath(error.to_string()))?;
        connection.batch_execute(
            "CREATE TABLE catalog_contents (
                 content_uuid BLOB PRIMARY KEY NOT NULL CHECK (length(content_uuid) = 16)
             );
             CREATE TABLE digest_values (
                 digest_id INTEGER PRIMARY KEY,
                 algorithm TEXT NOT NULL,
                 digest BLOB NOT NULL,
                 UNIQUE (algorithm, digest)
             );
             CREATE TABLE asset_occurrences (occurrence_id INTEGER PRIMARY KEY,content_uuid BLOB);
             CREATE TABLE occurrence_digest_assertions (
                 occurrence_id INTEGER,digest_id INTEGER,scope TEXT,provenance TEXT,
                 PRIMARY KEY (occurrence_id,digest_id,scope,provenance)
             );
             CREATE TABLE native_file_sizes (occurrence_id INTEGER PRIMARY KEY,size INTEGER);
             CREATE VIEW catalog_file_size_assertions AS SELECT * FROM native_file_sizes;
             CREATE VIEW accepted_file_size_assertions AS SELECT * FROM native_file_sizes;
             CREATE VIEW canonical_occurrence_content AS SELECT occurrence_id,content_uuid FROM asset_occurrences;
             CREATE TABLE merged_file_ids(old_content_uuid BLOB PRIMARY KEY, kept_content_uuid BLOB, decision_id INTEGER);
             CREATE TABLE file_match_decision_publications(decision_id INTEGER PRIMARY KEY);
             CREATE VIEW catalog_content_digest_assertions AS
             SELECT occurrence.content_uuid,assertion.* FROM occurrence_digest_assertions AS assertion
             JOIN asset_occurrences AS occurrence USING(occurrence_id)
             WHERE scope IN ('whole_asset','whole_file') AND provenance='source_declared';
             CREATE TABLE disputed_file_hashes(digest_id INTEGER,candidate_content_uuid BLOB);",
        )?;
        Ok(connection)
    }
}
