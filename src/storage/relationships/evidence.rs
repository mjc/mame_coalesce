//! Closed relationship decision facts, never a serialized source-record tree.

use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};

use super::RelationshipId;
use crate::{
    domain::RelationshipEvidence, reconciliation::ReconciliationStatus, resolution::EvidenceField,
};

#[derive(QueryableByName)]
struct Publication {
    #[diesel(sql_type = Text)]
    evidence_kind: String,
}

#[derive(QueryableByName)]
struct TextValue {
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(QueryableByName)]
struct FieldAssessment {
    #[diesel(sql_type = Text)]
    disposition: String,
    #[diesel(sql_type = BigInt)]
    list_order: i64,
    #[diesel(sql_type = Text)]
    field: String,
}

fn invalid(message: impl Into<String>) -> crate::Error {
    crate::Error::DatabaseSchema(message.into())
}

pub(super) fn insert(
    conn: &mut SqliteConnection,
    relationship: RelationshipId,
    evidence: &RelationshipEvidence,
) -> crate::Result<()> {
    match evidence {
        RelationshipEvidence::Rationale { reason } => {
            sql_query(
                "INSERT INTO catalog_relationship_rationales(relationship_id,reason) VALUES(?,?)",
            )
            .bind::<BigInt, _>(relationship.0)
            .bind::<Text, _>(reason)
            .execute(conn)?;
        }
        RelationshipEvidence::CatalogComparison {
            status,
            agreements,
            contradictions,
        } => {
            sql_query(
                "INSERT INTO catalog_relationship_comparisons(relationship_id,status) VALUES(?,?)",
            )
            .bind::<BigInt, _>(relationship.0)
            .bind::<Text, _>(status_code(*status))
            .execute(conn)?;
            for (disposition, fields) in
                [("agreement", agreements), ("contradiction", contradictions)]
            {
                for (position, field) in fields.iter().enumerate() {
                    sql_query("INSERT INTO catalog_relationship_comparison_fields(relationship_id,disposition,list_order,field) VALUES(?,?,?,?)")
                        .bind::<BigInt,_>(relationship.0)
                        .bind::<Text,_>(disposition)
                        .bind::<BigInt,_>(i64::try_from(position).map_err(|_| invalid("relationship field position exceeds SQLite range"))?)
                        .bind::<Text,_>(field_code(*field))
                        .execute(conn)?;
                }
            }
        }
        _ => {
            return Err(invalid(
                "source relationship evidence belongs to native catalog facts",
            ));
        }
    }
    Ok(())
}

pub(super) fn publish(
    conn: &mut SqliteConnection,
    relationship: RelationshipId,
    evidence: &RelationshipEvidence,
) -> crate::Result<()> {
    let kind = match evidence {
        RelationshipEvidence::Rationale { .. } => "rationale",
        RelationshipEvidence::CatalogComparison { .. } => "catalog_comparison",
        _ => {
            return Err(invalid(
                "source evidence cannot be copied into a decision publication",
            ));
        }
    };
    sql_query(
        "INSERT INTO catalog_relationship_evidence_publications(relationship_id,evidence_kind) VALUES(?,?)",
    )
    .bind::<BigInt, _>(relationship.0)
    .bind::<Text, _>(kind)
    .execute(conn)?;
    Ok(())
}

pub(super) fn load(
    conn: &mut SqliteConnection,
    relationship: RelationshipId,
) -> crate::Result<RelationshipEvidence> {
    let publication = sql_query(
        "SELECT evidence_kind FROM catalog_relationship_evidence_publications WHERE relationship_id=?",
    )
    .bind::<BigInt, _>(relationship.0)
    .get_result::<Publication>(conn)?;
    match publication.evidence_kind.as_str() {
        "rationale" => {
            let row = sql_query(
                "SELECT reason AS value FROM catalog_relationship_rationales WHERE relationship_id=?",
            )
            .bind::<BigInt, _>(relationship.0)
            .get_result::<TextValue>(conn)?;
            Ok(RelationshipEvidence::Rationale { reason: row.value })
        }
        "catalog_comparison" => {
            let row = sql_query(
                "SELECT status AS value FROM catalog_relationship_comparisons WHERE relationship_id=?",
            )
            .bind::<BigInt, _>(relationship.0)
            .get_result::<TextValue>(conn)?;
            let status = parse_status(&row.value)?;
            let fields = sql_query("SELECT disposition,list_order,field FROM catalog_relationship_comparison_fields WHERE relationship_id=? ORDER BY disposition,list_order")
                .bind::<BigInt,_>(relationship.0).load::<FieldAssessment>(conn)?;
            let mut agreements = Vec::new();
            let mut contradictions = Vec::new();
            for assessment in fields {
                let output = match assessment.disposition.as_str() {
                    "agreement" => &mut agreements,
                    "contradiction" => &mut contradictions,
                    _ => return Err(invalid("unknown relationship field disposition")),
                };
                if usize::try_from(assessment.list_order).ok() != Some(output.len()) {
                    return Err(invalid("relationship assessment order is not contiguous"));
                }
                output.push(parse_field(&assessment.field)?);
            }
            Ok(RelationshipEvidence::CatalogComparison {
                status,
                agreements,
                contradictions,
            })
        }
        _ => Err(invalid("unknown relationship evidence subtype")),
    }
}

const fn status_code(status: ReconciliationStatus) -> &'static str {
    match status {
        ReconciliationStatus::Compatible => "compatible",
        ReconciliationStatus::Candidate => "candidate",
        ReconciliationStatus::Contradictory => "contradictory",
        ReconciliationStatus::Ambiguous => "ambiguous",
        ReconciliationStatus::Unknown => "unknown",
    }
}

fn parse_status(status: &str) -> crate::Result<ReconciliationStatus> {
    match status {
        "compatible" => Ok(ReconciliationStatus::Compatible),
        "candidate" => Ok(ReconciliationStatus::Candidate),
        "contradictory" => Ok(ReconciliationStatus::Contradictory),
        "ambiguous" => Ok(ReconciliationStatus::Ambiguous),
        "unknown" => Ok(ReconciliationStatus::Unknown),
        _ => Err(invalid("unknown relationship comparison status")),
    }
}

const fn field_code(field: EvidenceField) -> &'static str {
    match field {
        EvidenceField::Sha1 => "sha1",
        EvidenceField::Md5 => "md5",
        EvidenceField::Crc => "crc",
        EvidenceField::Size => "size",
    }
}

fn parse_field(field: &str) -> crate::Result<EvidenceField> {
    match field {
        "sha1" => Ok(EvidenceField::Sha1),
        "md5" => Ok(EvidenceField::Md5),
        "crc" => Ok(EvidenceField::Crc),
        "size" => Ok(EvidenceField::Size),
        _ => Err(invalid("unknown relationship comparison field")),
    }
}
