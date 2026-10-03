use std::{error::Error, path::Path};

use camino::Utf8PathBuf;
use diesel::{
    QueryableByName, SqliteConnection,
    prelude::*,
    sql_query,
    sql_types::{BigInt, Binary, Text},
};
use mame_coalesce::{
    NoIntroDatMode,
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    catalog_files::OccurrenceId,
    database::Database,
    domain::{CatalogContentId, CatalogKey, CatalogScope, PublishingSourceKey},
    file_match_reviews::{
        self, ConflictRef, Disposition, EvidenceDecision, EvidenceRole, FileMatchReview,
        ReviewAction, ReviewedEvidence, SizeField, WholeFileScope,
    },
};

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct ConflictRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = Binary)]
    candidate_content_uuid: Vec<u8>,
}

#[derive(QueryableByName)]
struct HashEvidenceRow {
    #[diesel(sql_type = BigInt)]
    evidence_occurrence_id: i64,
    #[diesel(sql_type = BigInt)]
    digest_id: i64,
    #[diesel(sql_type = Text)]
    scope: String,
    #[diesel(sql_type = Text)]
    role: String,
    #[diesel(sql_type = Text)]
    algorithm: String,
}

#[derive(QueryableByName)]
struct SizeEvidenceRow {
    #[diesel(sql_type = BigInt)]
    evidence_occurrence_id: i64,
    #[diesel(sql_type = Text)]
    size_field: String,
    #[diesel(sql_type = Text)]
    role: String,
}

#[derive(QueryableByName)]
struct DiagnosticRow {
    #[diesel(sql_type = Text)]
    code: String,
    #[diesel(sql_type = Text)]
    message: String,
}

struct ImportedCatalogs {
    directory: tempfile::TempDir,
    database: Database,
    connection: SqliteConnection,
}

fn import_document(
    directory: &Path,
    database: &Database,
    source: &str,
    catalog: &str,
    format: CatalogDocumentFormat,
    filename: &str,
    contents: &str,
) -> Result<(), Box<dyn Error>> {
    let path = directory.join(filename);
    std::fs::write(&path, contents)?;
    let document_path = Utf8PathBuf::from_path_buf(path).map_err(|_| "non-UTF-8 input path")?;
    let report = app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path,
            format,
            source_key: PublishingSourceKey::new(source),
            source_display_name: source.to_owned(),
            catalog_key: CatalogKey::new(catalog),
            catalog_display_name: catalog.to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(
        report.status,
        app::CatalogImportStatus::Succeeded,
        "import of {filename} returned {} diagnostics: {:?}",
        report.diagnostic_count,
        sql_query(
            "SELECT diagnostic.code,diagnostic.message FROM import_diagnostics AS diagnostic \
             JOIN import_runs USING (run_key) WHERE import_runs.run_key = ? ORDER BY diagnostic_key",
        )
        .bind::<Text, _>(report.run_key.to_string())
        .load::<DiagnosticRow>(&mut SqliteConnection::establish(
            directory.join("catalog.sqlite").to_str().ok_or("non-UTF-8 database path")?,
        )?)?
        .into_iter()
        .map(|diagnostic| (diagnostic.code, diagnostic.message))
        .collect::<Vec<_>>()
    );
    Ok(())
}

fn content_id(bytes: Vec<u8>) -> Result<CatalogContentId, Box<dyn Error>> {
    let bytes: [u8; 16] = bytes
        .try_into()
        .map_err(|_| "catalog content ID is not a 16-byte UUID")?;
    Ok(CatalogContentId::from_bytes(bytes))
}

fn conflict_refs(
    connection: &mut SqliteConnection,
    minimum_candidates: i64,
) -> Result<(OccurrenceId, Vec<ConflictRef>), Box<dyn Error>> {
    let incoming = sql_query(
        "SELECT occurrence_id FROM occurrence_content_conflicts \
         GROUP BY occurrence_id HAVING COUNT(*) >= ? ORDER BY occurrence_id DESC LIMIT 1",
    )
    .bind::<BigInt, _>(minimum_candidates)
    .get_result::<OccurrenceRow>(connection)?
    .occurrence_id;
    let rows = sql_query(
        "SELECT occurrence_id,candidate_content_uuid FROM occurrence_content_conflicts \
         WHERE occurrence_id = ? ORDER BY candidate_content_uuid",
    )
    .bind::<BigInt, _>(incoming)
    .load::<ConflictRow>(connection)?;
    let conflicts = rows
        .into_iter()
        .map(|row| {
            Ok(ConflictRef {
                incoming: OccurrenceId::from_database(row.occurrence_id),
                candidate: content_id(row.candidate_content_uuid)?,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    Ok((OccurrenceId::from_database(incoming), conflicts))
}

#[derive(QueryableByName)]
struct OccurrenceRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
}

fn accepted_incoming_evidence(
    connection: &mut SqliteConnection,
    conflict: ConflictRef,
) -> Result<Vec<EvidenceDecision>, Box<dyn Error>> {
    let hashes = sql_query(
        "SELECT evidence.evidence_occurrence_id,evidence.digest_id,evidence.scope, \
                evidence.role,digest.algorithm \
         FROM occurrence_content_conflict_hashes AS evidence \
         JOIN digest_values AS digest USING (digest_id) \
         WHERE evidence.occurrence_id = ? AND evidence.candidate_content_uuid = ? \
           AND evidence.role = 'incoming' ORDER BY digest.algorithm",
    )
    .bind::<BigInt, _>(conflict.incoming.database_value())
    .bind::<Binary, _>(conflict.candidate.as_bytes().as_slice())
    .load::<HashEvidenceRow>(connection)?;
    let sizes = sql_query(
        "SELECT evidence.evidence_occurrence_id,evidence.size_field,evidence.role \
         FROM occurrence_content_conflict_sizes AS evidence \
         WHERE evidence.occurrence_id = ? AND evidence.candidate_content_uuid = ? \
           AND evidence.role = 'incoming' ORDER BY evidence.size_field",
    )
    .bind::<BigInt, _>(conflict.incoming.database_value())
    .bind::<Binary, _>(conflict.candidate.as_bytes().as_slice())
    .load::<SizeEvidenceRow>(connection)?;

    let mut evidence = hashes
        .into_iter()
        .map(|row| -> Result<_, Box<dyn Error>> {
            let scope = match row.scope.as_str() {
                "whole_asset" => WholeFileScope::WholeAsset,
                "whole_file" => WholeFileScope::WholeFile,
                other => {
                    return Err(format!("unexpected whole-file conflict scope {other}").into());
                }
            };
            assert_eq!(row.role, "incoming");
            assert!(matches!(
                row.algorithm.as_str(),
                "crc32" | "md5" | "sha1" | "sha256"
            ));
            Ok(EvidenceDecision {
                conflict,
                evidence: ReviewedEvidence::Hash {
                    occurrence: OccurrenceId::from_database(row.evidence_occurrence_id),
                    digest_id: row.digest_id,
                    scope,
                    role: EvidenceRole::Incoming,
                },
                disposition: Disposition::Accept,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    evidence.extend(
        sizes
            .into_iter()
            .map(|row| -> Result<_, Box<dyn Error>> {
                let field = match row.size_field.as_str() {
                    "mame_rom_size" => SizeField::MameRom,
                    "logiqx_rom_size" => SizeField::LogiqxRom,
                    "cmp_rom_size" => SizeField::ClrMameProRom,
                    "no_intro_pc_file_size" => SizeField::NoIntroPcFile,
                    "no_intro_dat_rom_size" => SizeField::NoIntroDatRom,
                    "no_intro_database_source_file_size" => SizeField::NoIntroDatabaseSourceFile,
                    "no_intro_database_release_file_size" => SizeField::NoIntroDatabaseReleaseFile,
                    other => return Err(format!("unexpected size evidence field {other}").into()),
                };
                assert_eq!(row.role, "incoming");
                Ok(EvidenceDecision {
                    conflict,
                    evidence: ReviewedEvidence::Size {
                        occurrence: OccurrenceId::from_database(row.evidence_occurrence_id),
                        field,
                        role: EvidenceRole::Incoming,
                    },
                    disposition: Disposition::Accept,
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
    );
    Ok(evidence)
}

fn import_no_intro_dat(
    directory: &Path,
    database: &Database,
    source: &str,
    catalog: &str,
    filename: &str,
    contents: &str,
) -> Result<(), Box<dyn Error>> {
    import_document(
        directory,
        database,
        source,
        catalog,
        CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
        filename,
        contents,
    )
}

fn imported_conflict() -> Result<ImportedCatalogs, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;

    // The Logiqx occurrence issues the candidate UUID. The No-Intro P/C
    // occurrence shares its SHA-1 while contradicting its MD5 and full-file size.
    import_document(
        directory.path(),
        &database,
        "review-candidate-source",
        "review-candidate-catalog",
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
        "candidate.dat",
        r#"<datafile><header><name>Review candidate</name></header>
          <game name="candidate"><rom name="candidate.bin" size="16"
            crc="12345678" md5="00112233445566778899aabbccddeeff"
            sha1="0123456789abcdef0123456789abcdef01234567"/>
          </game></datafile>"#,
    )?;
    import_document(
        directory.path(),
        &database,
        "review-incoming-source",
        "review-incoming-catalog",
        CatalogDocumentFormat::NoIntroPcXml,
        "incoming.xml",
        r#"<datafile><header><name>Review incoming</name><version>1</version>
          <description>Review incoming</description></header>
          <game name="incoming" id="7">
            <rom name="incoming.bin" size="32" crc="87654321"
              md5="ffeeddccbbaa99887766554433221100"
              sha1="0123456789abcdef0123456789abcdef01234567"/>
          </game></datafile>"#,
    )?;

    let connection = SqliteConnection::establish(database_path.as_str())?;
    Ok(ImportedCatalogs {
        directory,
        database,
        connection,
    })
}

struct MergeFixture {
    directory: tempfile::TempDir,
    database: Database,
    connection: SqliteConnection,
    incoming: OccurrenceId,
    conflicts: Vec<ConflictRef>,
}

fn imported_merge_conflict() -> Result<MergeFixture, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;

    // Two independently issued candidates share no identity digest with each
    // other: one has the incoming SHA-1, the other its SHA-256. Their size and
    // MD5 agree with each other, but contradict the incoming source declaration.
    import_document(
        directory.path(),
        &database,
        "review-merge-sha1-source",
        "review-merge-sha1-catalog",
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
        "candidate-sha1.xml",
        r#"<datafile><header><name>SHA-1 candidate</name></header>
          <game name="candidate-sha1"><rom name="a.bin" size="16"
            crc="12345678" md5="00112233445566778899aabbccddeeff"
            sha1="0123456789abcdef0123456789abcdef01234567"/>
          </game></datafile>"#,
    )?;
    import_no_intro_dat(
        directory.path(),
        &database,
        "review-merge-sha256-source",
        "review-merge-sha256-catalog",
        "candidate-sha256.dat",
        r#"<datafile><header><id>1</id><name>SHA-256 candidate</name>
          <description>SHA-256 candidate</description><version>1</version><author>test</author></header>
          <game name="candidate-sha256"><description>Candidate</description><rom name="b.bin" size="16"
            crc="12345678" md5="00112233445566778899aabbccddeeff"
            sha256="abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd"/>
          </game></datafile>"#,
    )?;
    import_no_intro_dat(
        directory.path(),
        &database,
        "review-merge-incoming-source",
        "review-merge-incoming-catalog",
        "incoming.dat",
        r#"<datafile><header><id>1</id><name>Ambiguous incoming</name>
          <description>Ambiguous incoming</description><version>1</version><author>test</author></header>
          <game name="incoming"><description>Incoming</description><rom name="incoming.bin" size="32"
            crc="87654321" md5="ffeeddccbbaa99887766554433221100"
            sha1="0123456789abcdef0123456789abcdef01234567"
            sha256="abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd"/>
          </game></datafile>"#,
    )?;

    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let (incoming, conflicts) = conflict_refs(&mut connection, 2)?;
    assert_eq!(conflicts.len(), 2, "both real imported aliases conflict");
    let incoming_link = sql_query(
        "SELECT COUNT(*) AS count FROM asset_occurrences \
         WHERE occurrence_id = ? AND content_uuid IS NOT NULL",
    )
    .bind::<BigInt, _>(incoming.database_value())
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(
        incoming_link.count, 0,
        "conflicted incoming remains unlinked"
    );

    Ok(MergeFixture {
        directory,
        database,
        connection,
        incoming,
        conflicts,
    })
}

fn all_incoming_evidence(
    fixture: &mut MergeFixture,
) -> Result<Vec<EvidenceDecision>, Box<dyn Error>> {
    let mut evidence = Vec::new();
    for conflict in fixture.conflicts.iter().copied() {
        evidence.extend(accepted_incoming_evidence(
            &mut fixture.connection,
            conflict,
        )?);
    }
    Ok(evidence)
}

#[derive(Clone, Copy)]
enum AcceptedFacts {
    HashOnly,
    SizeOnly,
    HashAndSize,
}

fn selected_incoming_evidence(
    fixture: &mut MergeFixture,
    accepted: AcceptedFacts,
) -> Result<Vec<EvidenceDecision>, Box<dyn Error>> {
    let evidence = all_incoming_evidence(fixture)?;
    let selected = evidence
        .into_iter()
        .filter(|decision| {
            matches!(
                (accepted, decision.evidence),
                (AcceptedFacts::HashOnly, ReviewedEvidence::Hash { .. })
                    | (AcceptedFacts::SizeOnly, ReviewedEvidence::Size { .. })
                    | (AcceptedFacts::HashAndSize, _)
            )
        })
        .collect::<Vec<_>>();
    Ok(selected)
}

fn review_merge(fixture: &MergeFixture, evidence: Vec<EvidenceDecision>) -> FileMatchReview {
    FileMatchReview {
        rationale: "Reviewing exact incoming/candidate conflict edges".into(),
        conflicts: fixture.conflicts.clone(),
        evidence,
        action: ReviewAction::Merge {
            kept: fixture.conflicts[0].candidate,
            old: vec![fixture.conflicts[1].candidate],
        },
    }
}

#[test]
fn accepted_incoming_contradictions_block_merge_without_promoting_incoming()
-> Result<(), Box<dyn Error>> {
    for accepted in [
        AcceptedFacts::HashOnly,
        AcceptedFacts::SizeOnly,
        AcceptedFacts::HashAndSize,
    ] {
        let mut fixture = imported_merge_conflict()?;
        let evidence = selected_incoming_evidence(&mut fixture, accepted)?;
        assert!(!evidence.is_empty());
        assert!(file_match_reviews::record_review(
            &fixture.database,
            &review_merge(&fixture, evidence),
        )
        .is_err(), "accepted incoming hash/size facts must independently remain merge constraints");
        let unpublished = sql_query("SELECT COUNT(*) AS count FROM file_match_decisions")
            .get_result::<CountRow>(&mut fixture.connection)?;
        assert_eq!(
            unpublished.count, 0,
            "failed publication rolls back atomically"
        );
    }
    Ok(())
}

#[test]
fn undispositioned_incoming_conflict_can_merge_without_promoting_incoming()
-> Result<(), Box<dyn Error>> {
    let mut fixture = imported_merge_conflict()?;
    let kept = fixture.conflicts[0].candidate;
    let old = fixture.conflicts[1].candidate;
    let review = review_merge(&fixture, Vec::new());
    let decision = file_match_reviews::record_review(&fixture.database, &review)?;

    let incoming_link = sql_query(
        "SELECT COUNT(*) AS count FROM asset_occurrences \
         WHERE occurrence_id = ? AND content_uuid IS NOT NULL",
    )
    .bind::<BigInt, _>(fixture.incoming.database_value())
    .get_result::<CountRow>(&mut fixture.connection)?;
    assert_eq!(
        incoming_link.count, 0,
        "merge never promotes its incoming occurrence"
    );
    let reviewed_facts = sql_query(
        "SELECT COUNT(*) AS count FROM file_match_hash_decisions \
         WHERE decision_id = ? UNION ALL SELECT COUNT(*) FROM file_match_size_decisions \
         WHERE decision_id = ?",
    )
    .bind::<BigInt, _>(decision.as_i64())
    .bind::<BigInt, _>(decision.as_i64())
    .load::<CountRow>(&mut fixture.connection)?;
    assert!(reviewed_facts.iter().all(|row| row.count == 0));
    assert_eq!(
        file_match_reviews::resolve_file_id(&fixture.database, old)?,
        kept
    );

    import_no_intro_dat(
        fixture.directory.path(),
        &fixture.database,
        "review-future-compatible-source",
        "review-future-compatible-catalog",
        "future-compatible.dat",
        r#"<datafile><header><id>1</id><name>Future compatible</name>
          <description>Future compatible</description><version>1</version><author>test</author></header>
          <game name="future"><description>Future</description><rom name="future.bin"
            sha1="0123456789abcdef0123456789abcdef01234567"/>
          </game></datafile>"#,
    )?;
    let future_link = sql_query(
        "SELECT COUNT(*) AS count FROM asset_occurrences AS occurrence \
         JOIN catalog_sets AS sets ON sets.set_id = occurrence.record_id \
         JOIN catalog_set_groups AS groups USING (set_group_id) \
         JOIN catalog_snapshots AS snapshot USING (snapshot_key) \
         WHERE snapshot.catalog_key = 'review-future-compatible-catalog' \
           AND occurrence.content_uuid = ?",
    )
    .bind::<Binary, _>(kept.as_bytes().as_slice())
    .get_result::<CountRow>(&mut fixture.connection)?;
    assert_eq!(
        future_link.count, 1,
        "future evidence resolves through canonical UUID"
    );
    Ok(())
}

#[test]
fn review_does_not_open_a_published_candidate_to_later_hash_assertions()
-> Result<(), Box<dyn Error>> {
    let mut imported = imported_conflict()?;
    let (_, conflicts) = conflict_refs(&mut imported.connection, 1)?;
    let conflict = conflicts[0];
    file_match_reviews::record_review(
        &imported.database,
        &FileMatchReview {
            rationale: "Keep this exact imported source conflict separate".into(),
            conflicts,
            evidence: Vec::new(),
            action: ReviewAction::KeepSeparate,
        },
    )?;

    let owner = sql_query(
        "SELECT evidence_occurrence_id AS occurrence_id \
         FROM occurrence_content_conflict_hashes \
         WHERE occurrence_id = ? AND candidate_content_uuid = ? AND role = 'candidate' \
         LIMIT 1",
    )
    .bind::<BigInt, _>(conflict.incoming.database_value())
    .bind::<Binary, _>(conflict.candidate.as_bytes().as_slice())
    .get_result::<OccurrenceRow>(&mut imported.connection)?
    .occurrence_id;
    let digest = [0xA5; 32];
    sql_query("INSERT INTO digest_values(algorithm,digest) VALUES ('sha256',?)")
        .bind::<Binary, _>(digest.as_slice())
        .execute(&mut imported.connection)?;
    let digest_id = sql_query(
        "SELECT digest_id AS occurrence_id FROM digest_values \
         WHERE algorithm='sha256' AND digest=?",
    )
    .bind::<Binary, _>(digest.as_slice())
    .get_result::<OccurrenceRow>(&mut imported.connection)?
    .occurrence_id;

    assert!(
        sql_query(
            "INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) \
             VALUES (?,?,'whole_file','source_declared')",
        )
        .bind::<BigInt, _>(owner)
        .bind::<BigInt, _>(digest_id)
        .execute(&mut imported.connection)
        .is_err(),
        "review publication must not permit contradictory facts to be appended to its source owner"
    );
    Ok(())
}

#[test]
fn keep_separate_settles_only_its_conflict_and_exact_rejection_restores_sparse_match()
-> Result<(), Box<dyn Error>> {
    let mut imported = imported_conflict()?;
    let (incoming, conflicts) = conflict_refs(&mut imported.connection, 1)?;
    let conflict = conflicts[0];
    let rejected_hash = sql_query(
        "SELECT evidence.evidence_occurrence_id,evidence.digest_id,evidence.scope, \
                evidence.role,digest.algorithm \
         FROM occurrence_content_conflict_hashes AS evidence \
         JOIN digest_values AS digest USING (digest_id) \
         WHERE evidence.occurrence_id = ? AND evidence.candidate_content_uuid = ? \
           AND evidence.role = 'candidate' AND digest.algorithm = 'md5' LIMIT 1",
    )
    .bind::<BigInt, _>(incoming.database_value())
    .bind::<Binary, _>(conflict.candidate.as_bytes().as_slice())
    .get_result::<HashEvidenceRow>(&mut imported.connection)?;
    assert_eq!(rejected_hash.scope, "whole_asset");
    assert_eq!(rejected_hash.role, "candidate");
    let rejected_size = sql_query(
        "SELECT evidence_occurrence_id,size_field,role \
         FROM occurrence_content_conflict_sizes \
         WHERE occurrence_id = ? AND candidate_content_uuid = ? AND role = 'candidate' LIMIT 1",
    )
    .bind::<BigInt, _>(incoming.database_value())
    .bind::<Binary, _>(conflict.candidate.as_bytes().as_slice())
    .get_result::<SizeEvidenceRow>(&mut imported.connection)?;
    assert_eq!(rejected_size.role, "candidate");
    assert_eq!(rejected_size.size_field, "logiqx_rom_size");

    file_match_reviews::record_review(
        &imported.database,
        &FileMatchReview {
            rationale: "Reject only the candidate's disputed MD5 assertion".into(),
            conflicts,
            evidence: vec![EvidenceDecision {
                conflict,
                evidence: ReviewedEvidence::Hash {
                    occurrence: OccurrenceId::from_database(rejected_hash.evidence_occurrence_id),
                    digest_id: rejected_hash.digest_id,
                    scope: WholeFileScope::WholeAsset,
                    role: EvidenceRole::Candidate,
                },
                disposition: Disposition::Reject,
            }],
            action: ReviewAction::KeepSeparate,
        },
    )?;

    assert_sparse_claim_matches(&mut imported, conflict.candidate)?;
    assert_later_size_conflict(&mut imported, conflict.candidate)?;
    Ok(())
}

fn assert_sparse_claim_matches(
    imported: &mut ImportedCatalogs,
    candidate: CatalogContentId,
) -> Result<(), Box<dyn Error>> {
    import_document(
        imported.directory.path(),
        &imported.database,
        "review-sparse-source",
        "review-sparse-catalog",
        CatalogDocumentFormat::NoIntroPcXml,
        "sparse.xml",
        r#"<datafile><header><name>Sparse later claim</name><version>1</version>
          <description>Sparse later claim</description></header>
          <game name="sparse" id="8"><rom name="sparse.bin"
            sha1="0123456789abcdef0123456789abcdef01234567"/>
          </game></datafile>"#,
    )?;
    let linked = sql_query(
        "SELECT COUNT(*) AS count FROM asset_occurrences AS occurrence \
         JOIN catalog_sets AS sets ON sets.set_id = occurrence.record_id \
         JOIN catalog_set_groups AS groups USING (set_group_id) \
         JOIN catalog_snapshots AS snapshot USING (snapshot_key) \
         WHERE snapshot.catalog_key = 'review-sparse-catalog' AND occurrence.content_uuid = ?",
    )
    .bind::<Binary, _>(candidate.as_bytes().as_slice())
    .get_result::<CountRow>(&mut imported.connection)?;
    assert_eq!(
        linked.count, 1,
        "sparse post-review claim resolves to candidate"
    );
    Ok(())
}

fn assert_later_size_conflict(
    imported: &mut ImportedCatalogs,
    candidate: CatalogContentId,
) -> Result<(), Box<dyn Error>> {
    import_document(
        imported.directory.path(),
        &imported.database,
        "review-size-still-conflicts-source",
        "review-size-still-conflicts-catalog",
        CatalogDocumentFormat::NoIntroPcXml,
        "size-still-conflicts.xml",
        r#"<datafile><header><name>Later contradictory claim</name><version>1</version>
          <description>Later contradictory claim</description></header>
          <game name="later" id="9"><rom name="later.bin" size="32"
            crc="87654321" md5="ffeeddccbbaa99887766554433221100"
            sha1="0123456789abcdef0123456789abcdef01234567"/>
          </game></datafile>"#,
    )?;
    let conflicts = sql_query(
        "SELECT COUNT(*) AS count FROM occurrence_content_conflicts AS conflict \
         JOIN catalog_sets AS sets ON sets.set_id = ( \
             SELECT record_id FROM asset_occurrences WHERE occurrence_id = conflict.occurrence_id) \
         JOIN catalog_set_groups AS groups USING (set_group_id) \
         JOIN catalog_snapshots AS snapshot USING (snapshot_key) \
         WHERE snapshot.catalog_key = 'review-size-still-conflicts-catalog' \
           AND conflict.candidate_content_uuid = ?",
    )
    .bind::<Binary, _>(candidate.as_bytes().as_slice())
    .get_result::<CountRow>(&mut imported.connection)?;
    assert_eq!(
        conflicts.count, 1,
        "unrejected size still blocks later claim"
    );
    Ok(())
}

#[test]
fn imported_cross_format_conflict_has_persistent_review_and_redirect_schema()
-> Result<(), Box<dyn Error>> {
    let mut imported = imported_conflict()?;
    let conflicts = sql_query("SELECT COUNT(*) AS count FROM occurrence_content_conflicts")
        .get_result::<CountRow>(&mut imported.connection)?;
    assert!(
        conflicts.count > 0,
        "both source formats imported a real conflict"
    );

    // The public API exercises these exact persisted decisions, terminal
    // conflicts, evidence, redirects, and final publication marker below.
    let review_storage = sql_query(
        "SELECT COUNT(*) AS count FROM sqlite_schema \
         WHERE type = 'table' AND name IN ( \
           'file_match_decisions', 'file_match_decision_conflicts', \
           'file_match_hash_decisions', 'file_match_size_decisions', \
           'merged_file_ids', 'file_match_decision_publications')",
    )
    .get_result::<CountRow>(&mut imported.connection)?;
    assert_eq!(
        review_storage.count, 6,
        "all exact reviewed-settlement and redirect tables are present"
    );
    Ok(())
}
