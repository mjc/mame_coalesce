use std::collections::{BTreeMap, BTreeSet};

use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Text},
    sqlite::Sqlite,
};

use super::{
    ContentDigestAssertions, ContentIdentityConflict, ContentIdentityInput,
    ContentIdentityResolution, DigestAssertion, content_id,
};
use crate::{
    domain::CatalogContentId,
    storage::{
        cached_sql::{OwnedBinding, cached_generated_sql},
        catalog_identity::OccurrenceId,
    },
};

mod staging;
use staging::staged_candidates;

const ASSERTION_BATCH: usize = 128;
const CANDIDATE_BATCH: usize = 128;

#[derive(QueryableByName)]
struct SizeRow {
    #[diesel(sql_type = Binary)]
    root_uuid: Vec<u8>,
    #[diesel(sql_type = BigInt)]
    size: i64,
}

#[derive(QueryableByName)]
struct FactRow {
    #[diesel(sql_type = Binary)]
    root_uuid: Vec<u8>,
    #[diesel(sql_type = Text)]
    algorithm: String,
    #[diesel(sql_type = Binary)]
    digest: Vec<u8>,
}

#[derive(Default)]
struct IdentityFacts {
    sizes: BTreeSet<i64>,
    digests: BTreeMap<String, BTreeSet<Vec<u8>>>,
}

#[derive(Default)]
struct ResolutionOverlay {
    hashes: BTreeMap<(String, Vec<u8>), CatalogContentId>,
    disputed_hashes: BTreeMap<(String, Vec<u8>), BTreeSet<CatalogContentId>>,
    disputed_contents: BTreeSet<CatalogContentId>,
    facts: BTreeMap<CatalogContentId, IdentityFacts>,
}

/// Resolutions for the normal prefix before a candidate set too wide to materialize.
///
/// If `has_wide_conflict` is true, the first unprocessed input is at
/// `resolutions.len()` and must be persisted unlinked before its conflict evidence is staged.
#[derive(Debug)]
pub struct ResolvedContentPrefix {
    pub resolutions: Vec<ContentIdentityResolution>,
    pub has_wide_conflict: bool,
}

/// Resolve one source-order prefix without materializing wide candidate sets in Rust.
///
/// Callers should pass bounded input prefixes and flush assertions before recording conflicts.
/// A wide conflict is not included in `resolutions`; the caller handles the input at
/// `resolutions.len()` with the staged conflict writer, then starts a fresh prefix afterward.
pub fn resolve_content_prefix(
    connection: &mut SqliteConnection,
    inputs: &[ContentIdentityInput<'_>],
) -> crate::Result<ResolvedContentPrefix> {
    resolve_batch(connection, inputs, &mut ResolutionOverlay::default())
}

/// Compatibility wrapper for prefixes that do not contain a wide conflict.
#[cfg(test)]
pub fn resolve_content_identities(
    connection: &mut SqliteConnection,
    inputs: &[ContentIdentityInput<'_>],
) -> crate::Result<Vec<ContentIdentityResolution>> {
    let prefix = resolve_content_prefix(connection, inputs)?;
    if prefix.has_wide_conflict {
        return Err(crate::Error::InvalidPath(
            "content identity prefix stopped before a wide conflict".to_owned(),
        ));
    }
    Ok(prefix.resolutions)
}

/// Persist all wide-conflict evidence directly from SQLite's staged candidate set.
pub fn record_wide_content_conflict(
    connection: &mut SqliteConnection,
    occurrence: OccurrenceId,
    assertions: ContentDigestAssertions<'_>,
) -> crate::Result<()> {
    staging::record_wide_conflict(connection, occurrence, assertions)
}

#[expect(
    clippy::too_many_lines,
    reason = "source-order identity decisions and overlay updates form one ordered algorithm"
)]
fn resolve_batch(
    connection: &mut SqliteConnection,
    inputs: &[ContentIdentityInput<'_>],
    overlay: &mut ResolutionOverlay,
) -> crate::Result<ResolvedContentPrefix> {
    let staged = staged_candidates(connection, inputs)?;
    let mut summaries = BTreeMap::new();
    for summary in staged {
        if summary.request_index >= inputs.len()
            || summaries.insert(summary.request_index, summary).is_some()
        {
            return Err(crate::Error::InvalidPath(
                "invalid or duplicate staged content candidate summary".to_owned(),
            ));
        }
    }
    let first_wide = summaries
        .iter()
        .find_map(|(index, summary)| (summary.count > 1).then_some(*index));
    let mut candidate_ids = BTreeSet::new();
    for summary in summaries.values() {
        if summary.count < 0 || (summary.count == 1) != summary.candidate.is_some() {
            return Err(crate::Error::InvalidPath(
                "inconsistent staged content candidate summary".to_owned(),
            ));
        }
        if summary.count == 1
            && !summary.disputed
            && first_wide.is_none_or(|wide_index| summary.request_index < wide_index)
            && let Some(candidate) = summary.candidate
        {
            candidate_ids.insert(candidate);
        }
    }
    let facts = load_facts(connection, &candidate_ids)?;
    let mut resolutions = Vec::with_capacity(inputs.len());
    let mut created = Vec::new();

    for (index, input) in inputs.iter().enumerate() {
        if !input.eligible {
            resolutions.push(ContentIdentityResolution::NoEligibleEvidence);
            continue;
        }
        validate_input(*input)?;
        let assertions = input.assertions;
        let whole_file = assertions
            .iter()
            .any(DigestAssertion::identifies_whole_file);
        if !whole_file {
            resolutions.push(ContentIdentityResolution::NoEligibleEvidence);
            continue;
        }

        let summary = summaries.remove(&index);
        if summary.as_ref().is_some_and(|summary| summary.count > 1) {
            if !created.is_empty() {
                insert_content_ids(connection, &created)?;
            }
            return Ok(ResolvedContentPrefix {
                resolutions,
                has_wide_conflict: true,
            });
        }
        let mut incoming_candidates = BTreeSet::new();
        let mut is_disputed = summary.as_ref().is_some_and(|summary| summary.disputed);
        if let Some(candidate) = summary.and_then(|summary| summary.candidate) {
            incoming_candidates.insert(candidate);
        }
        for assertion in assertions
            .iter()
            .filter(|assertion| assertion.identifies_whole_file())
        {
            let key = (
                assertion.algorithm.as_str().to_owned(),
                assertion.value.to_vec(),
            );
            if let Some(id) = overlay.hashes.get(&key) {
                incoming_candidates.insert(*id);
            }
            if let Some(ids) = overlay.disputed_hashes.get(&key) {
                incoming_candidates.extend(ids.iter().copied());
                is_disputed = true;
            }
        }
        if incoming_candidates.len() > 1 {
            if !created.is_empty() {
                insert_content_ids(connection, &created)?;
            }
            return Ok(ResolvedContentPrefix {
                resolutions,
                has_wide_conflict: true,
            });
        }
        is_disputed |= incoming_candidates
            .iter()
            .any(|candidate| overlay.disputed_contents.contains(candidate));

        let resolution = if is_disputed {
            ContentIdentityResolution::Conflict {
                reason: ContentIdentityConflict::DisputedAlias,
                candidates: incoming_candidates.into_iter().collect(),
            }
        } else {
            match incoming_candidates.len() {
                0 => {
                    let id = CatalogContentId::generate();
                    created.push(id);
                    ContentIdentityResolution::Linked(id)
                }
                1 => {
                    let id = *incoming_candidates.first().ok_or_else(|| {
                        crate::Error::InvalidPath("missing unique content candidate".to_owned())
                    })?;
                    if compatible_with_overlay(
                        facts.get(&id),
                        overlay.facts.get(&id),
                        input.size,
                        assertions,
                    ) {
                        ContentIdentityResolution::Linked(id)
                    } else {
                        ContentIdentityResolution::Conflict {
                            reason: ContentIdentityConflict::ContradictoryAssertions,
                            candidates: vec![id],
                        }
                    }
                }
                _ => ContentIdentityResolution::Conflict {
                    reason: ContentIdentityConflict::AmbiguousAlias,
                    candidates: incoming_candidates.into_iter().collect(),
                },
            }
        };

        if let ContentIdentityResolution::Linked(id) = resolution {
            if let Some(size) = input.size {
                overlay.facts.entry(id).or_default().sizes.insert(size);
            }
            let linked_facts = overlay.facts.entry(id).or_default();
            for assertion in assertions
                .iter()
                .filter(|assertion| matches!(assertion.scope, "whole_asset" | "whole_file"))
            {
                linked_facts
                    .digests
                    .entry(assertion.algorithm.as_str().to_owned())
                    .or_default()
                    .insert(assertion.value.to_vec());
            }
            for assertion in assertions
                .iter()
                .filter(|assertion| assertion.identifies_whole_file())
            {
                overlay.hashes.insert(
                    (
                        assertion.algorithm.as_str().to_owned(),
                        assertion.value.to_vec(),
                    ),
                    id,
                );
            }
            resolutions.push(ContentIdentityResolution::Linked(id));
        } else {
            if let ContentIdentityResolution::Conflict { candidates, .. } = &resolution {
                for candidate in candidates {
                    overlay.disputed_contents.insert(*candidate);
                    for assertion in assertions
                        .iter()
                        .filter(|assertion| assertion.identifies_whole_file())
                    {
                        overlay
                            .disputed_hashes
                            .entry((
                                assertion.algorithm.as_str().to_owned(),
                                assertion.value.to_vec(),
                            ))
                            .or_default()
                            .insert(*candidate);
                    }
                }
            }
            resolutions.push(resolution);
        }
    }

    if !created.is_empty() {
        insert_content_ids(connection, &created)?;
    }
    Ok(ResolvedContentPrefix {
        resolutions,
        has_wide_conflict: false,
    })
}

fn validate_input(input: ContentIdentityInput<'_>) -> crate::Result<()> {
    if input.size.is_some_and(|size| size < 0) {
        return Err(crate::Error::InvalidPath(
            "whole-file size cannot be negative".to_owned(),
        ));
    }
    for assertion in input.assertions.iter() {
        let expected = assertion.algorithm.byte_length();
        if assertion.value.len() != expected {
            return Err(crate::Error::InvalidHash(format!(
                "{} assertion contains {} bytes; expected {expected}",
                assertion.algorithm.as_str(),
                assertion.value.len()
            )));
        }
    }
    Ok(())
}

fn load_facts(
    connection: &mut SqliteConnection,
    ids: &BTreeSet<CatalogContentId>,
) -> crate::Result<BTreeMap<CatalogContentId, IdentityFacts>> {
    let mut facts = BTreeMap::new();
    let ids = ids.iter().copied().collect::<Vec<_>>();
    for batch in ids.chunks(CANDIDATE_BATCH) {
        facts.extend(load_facts_chunk(connection, batch)?);
    }
    Ok(facts)
}

fn load_facts_chunk(
    connection: &mut SqliteConnection,
    ids: &[CatalogContentId],
) -> crate::Result<BTreeMap<CatalogContentId, IdentityFacts>> {
    if ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let arity = ids.len().next_power_of_two();
    // Duplicate a real root, never a sentinel identity. UNION in component
    // deduplicates it while the finite arities keep expensive plans reusable.
    let roots = (0..arity)
        .map(|index| OwnedBinding::Binary(ids[index.min(ids.len() - 1)].as_bytes().to_vec()))
        .collect::<Vec<_>>();
    let values = std::iter::repeat_n("(?)", arity)
        .collect::<Vec<_>>()
        .join(", ");
    let component = format!(
        "WITH RECURSIVE roots(content_uuid) AS (VALUES {values}), \
         component(root_uuid, content_uuid) AS ( \
             SELECT content_uuid, content_uuid FROM roots UNION \
             SELECT component.root_uuid, redirect.old_content_uuid FROM component \
             JOIN merged_file_ids AS redirect ON redirect.kept_content_uuid = component.content_uuid \
             JOIN file_match_decision_publications USING (decision_id)) "
    );
    let sizes = cached_generated_sql(
        format!(
            "{component}, size_groups AS ( \
             SELECT component.root_uuid, MIN(sizes.size) AS minimum_size, MAX(sizes.size) AS maximum_size \
             FROM component CROSS JOIN asset_occurrences AS entry ON entry.content_uuid = component.content_uuid \
             CROSS JOIN accepted_file_size_assertions AS sizes ON sizes.occurrence_id = entry.occurrence_id \
             GROUP BY component.root_uuid) \
         SELECT root_uuid, minimum_size AS size FROM size_groups \
         UNION ALL \
         SELECT root_uuid, maximum_size AS size FROM size_groups \
         WHERE maximum_size <> minimum_size"
        ),
        roots.clone(),
    );
    let mut facts = BTreeMap::new();
    for row in sizes.load::<SizeRow>(connection)? {
        facts
            .entry(content_id(row.root_uuid)?)
            .or_insert_with(IdentityFacts::default)
            .sizes
            .insert(row.size);
    }

    let digests = cached_generated_sql(
        format!(
            "{component}, digest_groups AS ( \
             SELECT component.root_uuid, digest.algorithm, MIN(digest.digest) AS minimum_digest, \
                    MAX(digest.digest) AS maximum_digest \
             FROM component \
             CROSS JOIN asset_occurrences AS entry ON entry.content_uuid=component.content_uuid \
             CROSS JOIN catalog_content_digest_assertions AS assertion ON assertion.occurrence_id=entry.occurrence_id \
             CROSS JOIN digest_values AS digest ON digest.digest_id = assertion.digest_id \
             GROUP BY component.root_uuid, digest.algorithm) \
         SELECT root_uuid, algorithm, minimum_digest AS digest FROM digest_groups \
         UNION ALL \
         SELECT root_uuid, algorithm, maximum_digest AS digest FROM digest_groups \
         WHERE maximum_digest <> minimum_digest"
        ),
        roots,
    );
    for row in digests.load::<FactRow>(connection)? {
        facts
            .entry(content_id(row.root_uuid)?)
            .or_insert_with(IdentityFacts::default)
            .digests
            .entry(row.algorithm)
            .or_default()
            .insert(row.digest);
    }
    Ok(facts)
}

fn compatible_with_overlay(
    persisted: Option<&IdentityFacts>,
    overlay: Option<&IdentityFacts>,
    size: Option<i64>,
    assertions: ContentDigestAssertions<'_>,
) -> bool {
    let known_sizes = persisted
        .into_iter()
        .chain(overlay)
        .flat_map(|facts| facts.sizes.iter())
        .copied()
        .collect::<BTreeSet<_>>();
    if known_sizes.len() > 1
        || size.is_some_and(|incoming| known_sizes.iter().any(|fact| *fact != incoming))
    {
        return false;
    }
    let mut known_digests: BTreeMap<&str, BTreeSet<&[u8]>> = BTreeMap::new();
    for identity_facts in persisted.into_iter().chain(overlay) {
        for (algorithm, digests) in &identity_facts.digests {
            known_digests
                .entry(algorithm)
                .or_default()
                .extend(digests.iter().map(Vec::as_slice));
        }
    }
    if known_digests.values().any(|digests| digests.len() > 1) {
        return false;
    }
    assertions.iter().all(|assertion| {
        known_digests
            .get(assertion.algorithm.as_str())
            .is_none_or(|digests| digests.contains(assertion.value))
    })
}

fn insert_content_ids(
    connection: &mut SqliteConnection,
    ids: &[CatalogContentId],
) -> crate::Result<()> {
    // Generated identities are inserted in a single statement per input batch.
    let mut values = String::new();
    for _ in ids {
        if !values.is_empty() {
            values.push_str(", ");
        }
        values.push_str("(?)");
    }
    let mut query = sql_query(format!(
        "INSERT INTO catalog_contents(content_uuid) VALUES {values}"
    ))
    .into_boxed::<Sqlite>();
    for id in ids {
        query = query.bind::<Binary, _>(id.as_bytes().as_slice());
    }
    query.execute(connection)?;
    Ok(())
}

#[derive(Clone, Copy)]
struct AssertionRow<'a> {
    occurrence: OccurrenceId,
    assertion: DigestAssertion<'a>,
    provenance: &'a str,
}

/// Persist digest dictionary values and occurrence assertions with bounded bulk statements.
pub fn record_occurrence_digest_assertions_bulk(
    connection: &mut SqliteConnection,
    assertions: &[(OccurrenceId, ContentDigestAssertions<'_>)],
    provenance: &str,
) -> crate::Result<()> {
    let rows = assertions
        .iter()
        .flat_map(|(occurrence, digests)| {
            digests.iter().map(move |assertion| AssertionRow {
                occurrence: *occurrence,
                assertion,
                provenance,
            })
        })
        .collect::<Vec<_>>();
    for batch in rows.chunks(ASSERTION_BATCH) {
        persist_assertion_batch(connection, batch)?;
    }
    super::shared_facts::record_occurrences(
        connection,
        &assertions
            .iter()
            .map(|(occurrence, _)| *occurrence)
            .collect::<Vec<_>>(),
    )
}

fn persist_assertion_batch(
    connection: &mut SqliteConnection,
    rows: &[AssertionRow<'_>],
) -> crate::Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    let values = std::iter::repeat_n("(?, ?)", rows.len())
        .collect::<Vec<_>>()
        .join(", ");
    let mut dictionary = sql_query(format!(
        "INSERT INTO digest_values(algorithm, digest) VALUES {values} \
         ON CONFLICT(algorithm, digest) DO NOTHING"
    ))
    .into_boxed::<Sqlite>();
    for row in rows {
        dictionary = dictionary
            .bind::<Text, _>(row.assertion.algorithm.as_str())
            .bind::<Binary, _>(row.assertion.value);
    }
    dictionary.execute(connection)?;

    let values = std::iter::repeat_n("(?, ?, ?, ?, ?)", rows.len())
        .collect::<Vec<_>>()
        .join(", ");
    let mut insert = sql_query(format!(
        "WITH incoming(occurrence_id, algorithm, digest, scope, provenance) AS (VALUES {values}) \
         INSERT OR IGNORE INTO occurrence_digest_assertions \
         (occurrence_id, digest_id, scope, provenance) \
         SELECT incoming.occurrence_id, digest.digest_id, incoming.scope, incoming.provenance \
         FROM incoming \
         JOIN digest_values AS digest USING (algorithm, digest)"
    ))
    .into_boxed::<Sqlite>();
    for row in rows {
        insert = insert
            .bind::<BigInt, _>(row.occurrence.database_value())
            .bind::<Text, _>(row.assertion.algorithm.as_str())
            .bind::<Binary, _>(row.assertion.value)
            .bind::<Text, _>(row.assertion.scope)
            .bind::<Text, _>(row.provenance);
    }
    insert.execute(connection)?;
    Ok(())
}

#[cfg(test)]
mod tests;
