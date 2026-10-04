//! Native endpoint ownership; source-local literals never imply resolved files.

use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Bool, Nullable, Text},
};

use crate::domain::{
    CatalogContentId, CatalogRecordKind, CatalogRecordRef, CatalogSetId, ContentIdentity,
    NoIntroArchiveId, OccurrenceId, RelationshipEndpoint, RelationshipRule, SnapshotKey,
};

#[derive(QueryableByName)]
struct Identifier {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(QueryableByName)]
struct ValidOwner {
    #[diesel(sql_type = Bool)]
    valid: bool,
}

#[derive(QueryableByName)]
struct RuleRow {
    #[diesel(sql_type = BigInt)]
    rule_id: i64,
    #[diesel(sql_type = Text)]
    rule_key: String,
    #[diesel(sql_type = Text)]
    revision: String,
    #[diesel(sql_type = Text)]
    description: String,
}

/// A complete native endpoint, issued only after its concrete payload exists.
pub(super) struct TargetId(i64);

impl TargetId {
    pub(super) const fn database_value(&self) -> i64 {
        self.0
    }
}

pub(super) struct RuleId(i64);

impl RuleId {
    pub(super) const fn database_value(&self) -> i64 {
        self.0
    }
}

enum IntegerOwner {
    Set(CatalogSetId),
    Media(OccurrenceId),
    Archive(NoIntroArchiveId),
}

#[derive(Clone, Copy)]
enum DigestTarget {
    Declared,
    Observed,
}

impl DigestTarget {
    const fn kind(self) -> &'static str {
        match self {
            Self::Declared => "declared_digest",
            Self::Observed => "observed_content",
        }
    }

    const fn table(self) -> &'static str {
        match self {
            Self::Declared => "declared_digest_targets",
            Self::Observed => "observed_digest_targets",
        }
    }

    fn lookup_sql(self) -> String {
        format!(
            "SELECT target_id AS value FROM {} WHERE digest_id=?",
            self.table()
        )
    }
}

fn digest_target(
    conn: &mut SqliteConnection,
    identity: &ContentIdentity,
    owner: DigestTarget,
) -> crate::Result<TargetId> {
    let digest = hex::decode(identity.digest())
        .map_err(|error| crate::Error::InvalidHash(error.to_string()))?;
    let digest_id = crate::storage::catalog_content::intern_digest(
        conn,
        identity.algorithm().as_str(),
        &digest,
    )?;
    let table = owner.table();
    if let Some(row) = sql_query(owner.lookup_sql())
        .bind::<BigInt, _>(digest_id)
        .get_result::<Identifier>(conn)
        .optional()?
    {
        return Ok(TargetId(row.value));
    }
    let target = create_target(conn, owner.kind())?;
    sql_query(format!(
        "INSERT INTO {table}(target_id,digest_id) VALUES(?,?)"
    ))
    .bind::<BigInt, _>(target)
    .bind::<BigInt, _>(digest_id)
    .execute(conn)?;
    Ok(TargetId(target))
}

impl IntegerOwner {
    const fn kind(&self) -> &'static str {
        match self {
            Self::Set(_) => "catalog_set",
            Self::Media(_) => "catalog_media_entry",
            Self::Archive(_) => "no_intro_archive",
        }
    }

    const fn table_and_column(&self) -> (&'static str, &'static str) {
        match self {
            Self::Set(_) => ("catalog_set_targets", "set_id"),
            Self::Media(_) => ("catalog_media_entry_targets", "occurrence_id"),
            Self::Archive(_) => ("no_intro_archive_targets", "archive_id"),
        }
    }

    const fn id(&self) -> i64 {
        match self {
            Self::Set(id) => id.as_i64(),
            Self::Media(id) => id.database_value(),
            Self::Archive(id) => id.as_i64(),
        }
    }
}

fn invalid(message: &str) -> crate::Error {
    crate::Error::InvalidPath(message.into())
}

fn create_target(conn: &mut SqliteConnection, kind: &str) -> crate::Result<i64> {
    Ok(sql_query(
        "INSERT INTO catalog_relationship_targets(kind) VALUES(?) RETURNING target_id AS value",
    )
    .bind::<Text, _>(kind)
    .get_result::<Identifier>(conn)?
    .value)
}

fn integer_target(conn: &mut SqliteConnection, owner: &IntegerOwner) -> crate::Result<TargetId> {
    let (table, column) = owner.table_and_column();
    if let Some(row) = sql_query(format!(
        "SELECT target_id AS value FROM {table} WHERE {column}=?"
    ))
    .bind::<BigInt, _>(owner.id())
    .get_result::<Identifier>(conn)
    .optional()?
    {
        return Ok(TargetId(row.value));
    }
    let id = create_target(conn, owner.kind())?;
    sql_query(format!(
        "INSERT INTO {table}(target_id,{column}) VALUES(?,?)"
    ))
    .bind::<BigInt, _>(id)
    .bind::<BigInt, _>(owner.id())
    .execute(conn)?;
    Ok(TargetId(id))
}

fn validate_native_edition(
    conn: &mut SqliteConnection,
    owner: &IntegerOwner,
    snapshot: &SnapshotKey,
) -> crate::Result<()> {
    let owner_query = match owner {
        IntegerOwner::Set(_) => {
            "SELECT sets.set_group_id FROM catalog_sets sets WHERE sets.set_id=?"
        }
        IntegerOwner::Media(_) => {
            "SELECT sets.set_group_id FROM asset_occurrences media JOIN catalog_sets sets ON sets.set_id=media.record_id WHERE media.occurrence_id=?"
        }
        IntegerOwner::Archive(_) => {
            "SELECT sets.set_group_id FROM no_intro_archive_descriptions archive JOIN catalog_sets sets USING(set_id) WHERE archive.archive_id=?"
        }
    };
    let row = sql_query(format!("SELECT EXISTS(SELECT 1 FROM catalog_set_groups groups JOIN snapshot_publications publication USING(snapshot_key) WHERE groups.set_group_id=({owner_query}) AND groups.snapshot_key=?) AS valid"))
        .bind::<BigInt,_>(owner.id()).bind::<Text,_>(snapshot.as_str()).get_result::<ValidOwner>(conn)?;
    if !row.valid {
        return Err(invalid(
            "relationship target requires its actual published catalog owner and edition",
        ));
    }
    Ok(())
}

enum RecordDeclaration {
    Set {
        name: String,
    },
    Software {
        list: String,
        name: String,
    },
    Media {
        set: String,
        name: String,
        order: i64,
    },
}

impl RecordDeclaration {
    fn parse(record: &CatalogRecordRef) -> crate::Result<Self> {
        match record.kind {
            CatalogRecordKind::Set => Ok(Self::Set {
                name: record.key.as_str().into(),
            }),
            CatalogRecordKind::SoftwareItem => {
                let (list, name) = serde_json::from_str::<(String, String)>(record.key.as_str())?;
                Ok(Self::Software { list, name })
            }
            CatalogRecordKind::AssetRequirement => {
                let (set, name, order) =
                    serde_json::from_str::<(String, String, i64)>(record.key.as_str())?;
                if order < 0 {
                    return Err(invalid(
                        "unresolved media declaration order must be nonnegative",
                    ));
                }
                Ok(Self::Media { set, name, order })
            }
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Set { name } | Self::Software { name, .. } | Self::Media { name, .. } => name,
        }
    }
    fn set(&self) -> Option<&str> {
        match self {
            Self::Media { set, .. } => Some(set),
            _ => None,
        }
    }
    fn list(&self) -> Option<&str> {
        match self {
            Self::Software { list, .. } => Some(list),
            _ => None,
        }
    }
    const fn order(&self) -> Option<i64> {
        match self {
            Self::Media { order, .. } => Some(*order),
            _ => None,
        }
    }
}

fn record_target(
    conn: &mut SqliteConnection,
    record: &CatalogRecordRef,
) -> crate::Result<TargetId> {
    let declaration = RecordDeclaration::parse(record)?;
    if let Some(set) = record.owner_set_id {
        if record.kind == CatalogRecordKind::AssetRequirement {
            return Err(invalid(
                "resolved media targets require an actual occurrence ID, not a name/order tuple",
            ));
        }
        let valid = sql_query("SELECT EXISTS(SELECT 1 FROM catalog_sets sets JOIN catalog_set_groups groups USING(set_group_id) LEFT JOIN software_lists list ON list.namespace_id=groups.set_group_id JOIN snapshot_publications publication USING(snapshot_key) WHERE sets.set_id=? AND groups.snapshot_key=? AND sets.set_name=? AND ((?='catalog_set' AND groups.kind='root') OR (?='software_item' AND groups.kind='software_list' AND list.name=?))) AS valid")
            .bind::<BigInt,_>(set.as_i64()).bind::<Text,_>(record.snapshot.as_str())
            .bind::<Text,_>(declaration.name()).bind::<Text,_>(record.kind.as_str())
            .bind::<Text,_>(record.kind.as_str()).bind::<Nullable<Text>,_>(declaration.list())
            .get_result::<ValidOwner>(conn)?;
        if !valid.valid {
            return Err(invalid(
                "catalog set owner does not match its endpoint and published edition",
            ));
        }
        return integer_target(conn, &IntegerOwner::Set(set));
    }
    let existing = sql_query("SELECT target_id AS value FROM unresolved_catalog_targets WHERE snapshot_key=? AND record_kind=? AND declared_name=? AND declared_set_name IS ? AND declared_list_name IS ? AND declared_media_order IS ?")
        .bind::<Text,_>(record.snapshot.as_str()).bind::<Text,_>(record.kind.as_str())
        .bind::<Text,_>(declaration.name()).bind::<Nullable<Text>,_>(declaration.set())
        .bind::<Nullable<Text>,_>(declaration.list()).bind::<Nullable<BigInt>,_>(declaration.order())
        .get_result::<Identifier>(conn).optional()?;
    if let Some(row) = existing {
        return Ok(TargetId(row.value));
    }
    let target = create_target(conn, "unresolved_catalog")?;
    sql_query("INSERT INTO unresolved_catalog_targets(target_id,snapshot_key,record_kind,declared_name,declared_set_name,declared_list_name,declared_media_order) VALUES(?,?,?,?,?,?,?)")
        .bind::<BigInt,_>(target).bind::<Text,_>(record.snapshot.as_str()).bind::<Text,_>(record.kind.as_str())
        .bind::<Text,_>(declaration.name()).bind::<Nullable<Text>,_>(declaration.set())
        .bind::<Nullable<Text>,_>(declaration.list()).bind::<Nullable<BigInt>,_>(declaration.order()).execute(conn)?;
    Ok(TargetId(target))
}

fn shared_file_target(
    conn: &mut SqliteConnection,
    file: CatalogContentId,
) -> crate::Result<TargetId> {
    if let Some(row) =
        sql_query("SELECT target_id AS value FROM shared_file_targets WHERE content_uuid=?")
            .bind::<Binary, _>(file.as_bytes().as_slice())
            .get_result::<Identifier>(conn)
            .optional()?
    {
        return Ok(TargetId(row.value));
    }
    let target = create_target(conn, "shared_file")?;
    sql_query("INSERT INTO shared_file_targets(target_id,content_uuid) VALUES(?,?)")
        .bind::<BigInt, _>(target)
        .bind::<Binary, _>(file.as_bytes().as_slice())
        .execute(conn)?;
    Ok(TargetId(target))
}

pub(super) fn insert(
    conn: &mut SqliteConnection,
    endpoint: &RelationshipEndpoint,
) -> crate::Result<TargetId> {
    match endpoint {
        RelationshipEndpoint::CatalogRecord(record) => record_target(conn, record),
        RelationshipEndpoint::CatalogMediaEntry {
            snapshot,
            occurrence_id,
        } => {
            let owner = IntegerOwner::Media(*occurrence_id);
            validate_native_edition(conn, &owner, snapshot)?;
            integer_target(conn, &owner)
        }
        RelationshipEndpoint::NoIntroArchive {
            snapshot,
            archive_id,
        } => {
            let owner = IntegerOwner::Archive(*archive_id);
            validate_native_edition(conn, &owner, snapshot)?;
            integer_target(conn, &owner)
        }
        RelationshipEndpoint::SharedCatalogFile(file) => shared_file_target(conn, *file),
        RelationshipEndpoint::ExternalRecord(record) => {
            if let Some(row)=sql_query("SELECT target_id AS value FROM external_catalog_targets WHERE namespace=? AND declared_key=?")
                .bind::<Text,_>(&record.namespace).bind::<Text,_>(record.key.as_str()).get_result::<Identifier>(conn).optional()? { return Ok(TargetId(row.value)); }
            let target = create_target(conn, "external_record")?;
            sql_query("INSERT INTO external_catalog_targets(target_id,namespace,declared_key) VALUES(?,?,?)")
                .bind::<BigInt,_>(target).bind::<Text,_>(&record.namespace).bind::<Text,_>(record.key.as_str()).execute(conn)?;
            Ok(TargetId(target))
        }
        RelationshipEndpoint::ContentObject(identity) => {
            digest_target(conn, identity, DigestTarget::Declared)
        }
        RelationshipEndpoint::ObservedContent(identity) => {
            digest_target(conn, identity.identity(), DigestTarget::Observed)
        }
        RelationshipEndpoint::CatalogMergeReference { .. }
        | RelationshipEndpoint::NoIntroArchiveReference { .. }
        | RelationshipEndpoint::NoIntroDatIdReference { .. } => Err(invalid(
            "source-local references retain their reported declaration identity; use its support key",
        )),
    }
}

pub(super) fn insert_rule(
    conn: &mut SqliteConnection,
    rule: &RelationshipRule,
) -> crate::Result<RuleId> {
    if let Some(existing) = sql_query("SELECT rule_id,rule_key,revision,description FROM catalog_relationship_rules WHERE rule_key=? AND revision=?")
        .bind::<Text, _>(rule.key())
        .bind::<Text, _>(rule.revision())
        .get_result::<RuleRow>(conn)
        .optional()?
    {
        if existing.description != rule.description() {
            return Err(invalid("relationship rule revision already has a different description"));
        }
        return Ok(RuleId(existing.rule_id));
    }
    Ok(RuleId(sql_query("INSERT INTO catalog_relationship_rules(rule_key,revision,description) VALUES(?,?,?) RETURNING rule_id AS value")
        .bind::<Text,_>(rule.key()).bind::<Text,_>(rule.revision()).bind::<Text,_>(rule.description()).get_result::<Identifier>(conn)?.value))
}

pub(super) fn load_rule(
    conn: &mut SqliteConnection,
    relationship: super::RelationshipId,
) -> crate::Result<RelationshipRule> {
    let row = sql_query("SELECT rule.rule_id,rule.rule_key,rule.revision,rule.description FROM inferred_catalog_relationships inferred JOIN catalog_relationship_rules rule USING(rule_id) WHERE inferred.relationship_id=?")
        .bind::<BigInt, _>(relationship.0)
        .get_result::<RuleRow>(conn)?;
    RelationshipRule::new(row.rule_key, row.revision, row.description)
}

#[cfg(test)]
mod digest_plan_tests;
