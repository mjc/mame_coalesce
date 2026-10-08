//! Immutable identities for the rules used to interpret a retained catalog.

use diesel::{
    Connection, OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};

use crate::storage::catalog_ids::ReadingRulesId;

/// One of the catalog families represented by `catalog_reading_rules`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatFamily {
    Mame,
    Software,
    Logiqx,
    ClrMamePro,
    NoIntroDat,
    NoIntroDatabase,
    NoIntroPcFixture,
}

impl FormatFamily {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Mame => "mame",
            Self::Software => "software",
            Self::Logiqx => "logiqx",
            Self::ClrMamePro => "clrmamepro",
            Self::NoIntroDat => "no_intro_dat",
            Self::NoIntroDatabase => "no_intro_database",
            Self::NoIntroPcFixture => "no_intro_pc_fixture",
        }
    }

    fn from_str(value: &str) -> crate::Result<Self> {
        match value {
            "mame" => Ok(Self::Mame),
            "software" => Ok(Self::Software),
            "logiqx" => Ok(Self::Logiqx),
            "clrmamepro" => Ok(Self::ClrMamePro),
            "no_intro_dat" => Ok(Self::NoIntroDat),
            "no_intro_database" => Ok(Self::NoIntroDatabase),
            "no_intro_pc_fixture" => Ok(Self::NoIntroPcFixture),
            _ => Err(invalid_persisted_value(
                "catalog_reading_rules.format_family",
                value,
            )),
        }
    }
}

/// A declared complete-file byte role supported by the catalog schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileByteContract {
    Mame0289MachineRom,
    Mame0289SoftwareFile,
    LogiqxCompleteDeclaredFile,
    ClrMameProDeclaredAsset,
    NoIntroDatUnfilteredFile,
    NoIntroPcFixtureAsset,
}

impl FileByteContract {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Mame0289MachineRom => "mame_0289_machine_rom",
            Self::Mame0289SoftwareFile => "mame_0289_software_file",
            Self::LogiqxCompleteDeclaredFile => "logiqx_complete_declared_file",
            Self::ClrMameProDeclaredAsset => "clrmamepro_declared_asset",
            Self::NoIntroDatUnfilteredFile => "no_intro_dat_unfiltered_file",
            Self::NoIntroPcFixtureAsset => "no_intro_pc_fixture_asset",
        }
    }

    const fn format_family(self) -> FormatFamily {
        match self {
            Self::Mame0289MachineRom => FormatFamily::Mame,
            Self::Mame0289SoftwareFile => FormatFamily::Software,
            Self::LogiqxCompleteDeclaredFile => FormatFamily::Logiqx,
            Self::ClrMameProDeclaredAsset => FormatFamily::ClrMamePro,
            Self::NoIntroDatUnfilteredFile => FormatFamily::NoIntroDat,
            Self::NoIntroPcFixtureAsset => FormatFamily::NoIntroPcFixture,
        }
    }

    fn from_str(value: &str) -> crate::Result<Self> {
        match value {
            "mame_0289_machine_rom" => Ok(Self::Mame0289MachineRom),
            "mame_0289_software_file" => Ok(Self::Mame0289SoftwareFile),
            "logiqx_complete_declared_file" => Ok(Self::LogiqxCompleteDeclaredFile),
            "clrmamepro_declared_asset" => Ok(Self::ClrMameProDeclaredAsset),
            "no_intro_dat_unfiltered_file" => Ok(Self::NoIntroDatUnfilteredFile),
            "no_intro_pc_fixture_asset" => Ok(Self::NoIntroPcFixtureAsset),
            _ => Err(invalid_persisted_value(
                "catalog_file_byte_contracts.contract_kind",
                value,
            )),
        }
    }
}

/// The complete immutable policy identified by one stable `rules_key`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadingRulesSpec {
    pub rules_key: String,
    pub format_family: FormatFamily,
    pub dialect: String,
    pub specification_version: String,
    pub parser_version: String,
    pub rules_version: String,
    /// `None` means no XML repair facet; `Some(false)` records an explicit no-repair policy.
    pub replace_nul: Option<bool>,
    /// `None` means no complete-file byte contract has been asserted.
    pub file_byte_contract: Option<FileByteContract>,
}

#[derive(QueryableByName)]
struct ReadingRulesRow {
    #[diesel(sql_type = BigInt)]
    reading_rules_id: i64,
    #[diesel(sql_type = Text)]
    rules_key: String,
    #[diesel(sql_type = Text)]
    format_family: String,
    #[diesel(sql_type = Text)]
    dialect: String,
    #[diesel(sql_type = Text)]
    specification_version: String,
    #[diesel(sql_type = Text)]
    parser_version: String,
    #[diesel(sql_type = Text)]
    rules_version: String,
}

#[derive(QueryableByName)]
struct ReadingRulesIdRow {
    #[diesel(sql_type = BigInt)]
    reading_rules_id: i64,
}

#[derive(QueryableByName)]
struct BooleanFacetRow {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(QueryableByName)]
struct ByteContractRow {
    #[diesel(sql_type = Text)]
    contract_kind: String,
}

/// Issue an immutable rules identity and its optional policy facets atomically.
///
/// The caller can reference the returned ID from an import or edition after this
/// operation. A key already issued for another policy is an error; policy
/// changes require a new key. The connection may already be in an outer
/// transaction, in which case Diesel uses a nested savepoint.
pub fn issue(
    connection: &mut SqliteConnection,
    specification: &ReadingRulesSpec,
) -> crate::Result<ReadingRulesId> {
    validate_specification(specification)?;

    connection.transaction::<_, crate::Error, _>(|connection| {
        let existing = sql_query(
            "SELECT reading_rules_id, rules_key, format_family, dialect, specification_version, \
                    parser_version, rules_version \
             FROM catalog_reading_rules WHERE rules_key = ?",
        )
        .bind::<Text, _>(&specification.rules_key)
        .get_result::<ReadingRulesRow>(connection)
        .optional()?;

        if let Some(existing) = existing {
            let id = reading_rules_id(existing.reading_rules_id)?;
            if !same_base_policy(&existing, specification)?
                || load_replace_nul(connection, id)? != specification.replace_nul
                || load_file_byte_contract(connection, id)? != specification.file_byte_contract
            {
                return Err(policy_conflict(&specification.rules_key));
            }
            return Ok(id);
        }

        sql_query(
            "INSERT INTO catalog_reading_rules \
             (rules_key, format_family, dialect, specification_version, parser_version, rules_version) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(&specification.rules_key)
        .bind::<Text, _>(specification.format_family.as_str())
        .bind::<Text, _>(&specification.dialect)
        .bind::<Text, _>(&specification.specification_version)
        .bind::<Text, _>(&specification.parser_version)
        .bind::<Text, _>(&specification.rules_version)
        .execute(connection)?;

        let id = sql_query("SELECT last_insert_rowid() AS reading_rules_id")
            .get_result::<ReadingRulesIdRow>(connection)
            .map(|row| reading_rules_id(row.reading_rules_id))??;

        if let Some(replace_nul) = specification.replace_nul {
            sql_query(
                "INSERT INTO catalog_xml_repairs (reading_rules_id, replace_nul) VALUES (?, ?)",
            )
            .bind::<BigInt, _>(id.as_i64())
            .bind::<BigInt, _>(if replace_nul { 1_i64 } else { 0_i64 })
            .execute(connection)?;
        }

        if let Some(contract) = specification.file_byte_contract {
            sql_query(
                "INSERT INTO catalog_file_byte_contracts (reading_rules_id, contract_kind) \
                 VALUES (?, ?)",
            )
            .bind::<BigInt, _>(id.as_i64())
            .bind::<Text, _>(contract.as_str())
            .execute(connection)?;
        }

        Ok(id)
    })
}

/// Load the complete immutable interpretation, including its optional policy facets.
pub fn load(
    connection: &mut SqliteConnection,
    id: ReadingRulesId,
) -> crate::Result<ReadingRulesSpec> {
    let row = sql_query(
        "SELECT reading_rules_id, rules_key, format_family, dialect, specification_version, \
                parser_version, rules_version FROM catalog_reading_rules WHERE reading_rules_id=?",
    )
    .bind::<BigInt, _>(id.as_i64())
    .get_result::<ReadingRulesRow>(connection)?;
    let specification = ReadingRulesSpec {
        rules_key: row.rules_key,
        format_family: FormatFamily::from_str(&row.format_family)?,
        dialect: row.dialect,
        specification_version: row.specification_version,
        parser_version: row.parser_version,
        rules_version: row.rules_version,
        replace_nul: load_replace_nul(connection, id)?,
        file_byte_contract: load_file_byte_contract(connection, id)?,
    };
    validate_specification(&specification)?;
    Ok(specification)
}

fn validate_specification(specification: &ReadingRulesSpec) -> crate::Result<()> {
    if let Some(contract) = specification.file_byte_contract
        && contract.format_family() != specification.format_family
    {
        return Err(crate::Error::DatabaseSchema(format!(
            "file byte contract {} does not match format family {}",
            contract.as_str(),
            specification.format_family.as_str()
        )));
    }

    if specification.replace_nul.is_some()
        && specification.format_family != FormatFamily::NoIntroDatabase
    {
        return Err(crate::Error::DatabaseSchema(
            "XML NUL-repair policy is only defined for No-Intro database exports".to_owned(),
        ));
    }

    Ok(())
}

fn same_base_policy(
    existing: &ReadingRulesRow,
    specification: &ReadingRulesSpec,
) -> crate::Result<bool> {
    Ok(
        FormatFamily::from_str(&existing.format_family)? == specification.format_family
            && existing.dialect == specification.dialect
            && existing.specification_version == specification.specification_version
            && existing.parser_version == specification.parser_version
            && existing.rules_version == specification.rules_version,
    )
}

fn load_replace_nul(
    connection: &mut SqliteConnection,
    id: ReadingRulesId,
) -> crate::Result<Option<bool>> {
    let value = sql_query(
        "SELECT replace_nul AS value FROM catalog_xml_repairs WHERE reading_rules_id = ?",
    )
    .bind::<BigInt, _>(id.as_i64())
    .get_result::<BooleanFacetRow>(connection)
    .optional()?
    .map(|row| match row.value {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(invalid_persisted_value(
            "catalog_xml_repairs.replace_nul",
            &value.to_string(),
        )),
    })
    .transpose()?;
    Ok(value)
}

fn load_file_byte_contract(
    connection: &mut SqliteConnection,
    id: ReadingRulesId,
) -> crate::Result<Option<FileByteContract>> {
    let value = sql_query(
        "SELECT contract_kind FROM catalog_file_byte_contracts WHERE reading_rules_id = ?",
    )
    .bind::<BigInt, _>(id.as_i64())
    .get_result::<ByteContractRow>(connection)
    .optional()?
    .map(|row| FileByteContract::from_str(&row.contract_kind))
    .transpose()?;
    Ok(value)
}

fn reading_rules_id(value: i64) -> crate::Result<ReadingRulesId> {
    ReadingRulesId::try_from(value).map_err(|error| crate::Error::DatabaseSchema(error.to_string()))
}

fn invalid_persisted_value(field: &str, value: &str) -> crate::Error {
    crate::Error::DatabaseSchema(format!("invalid persisted {field}: {value}"))
}

fn policy_conflict(rules_key: &str) -> crate::Error {
    crate::Error::DatabaseSchema(format!(
        "reading rules key {rules_key:?} is already issued with a different policy"
    ))
}

#[cfg(test)]
mod tests {
    use diesel::{QueryableByName, RunQueryDsl, sql_query, sql_types::BigInt};

    use super::*;

    fn database() -> crate::Result<crate::database::Database> {
        crate::database::Database::in_memory()
    }

    fn mame_rules(key: &str) -> ReadingRulesSpec {
        ReadingRulesSpec {
            rules_key: key.to_owned(),
            format_family: FormatFamily::Mame,
            dialect: "observed".to_owned(),
            specification_version: "0.289".to_owned(),
            parser_version: "mame_coalesce-test".to_owned(),
            rules_version: "mame-observed-compat-declared-text-v2".to_owned(),
            replace_nul: None,
            file_byte_contract: Some(FileByteContract::Mame0289MachineRom),
        }
    }

    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = BigInt)]
        count: i64,
    }

    fn count_rows(connection: &mut diesel::SqliteConnection, table: &str) -> crate::Result<i64> {
        let query = match table {
            "catalog_reading_rules" => "SELECT count(*) AS count FROM catalog_reading_rules",
            "catalog_xml_repairs" => "SELECT count(*) AS count FROM catalog_xml_repairs",
            "catalog_file_byte_contracts" => {
                "SELECT count(*) AS count FROM catalog_file_byte_contracts"
            }
            _ => unreachable!("test asks only for a known reading-rules table"),
        };
        Ok(sql_query(query).get_result::<Count>(connection)?.count)
    }

    #[test]
    fn identical_rules_key_and_policy_reuse_the_issued_id() -> crate::Result<()> {
        let database = database()?;
        let mut connection = database.pool().get()?;
        let specification = mame_rules("mame/0.289/observed-v2");

        let first = issue(&mut connection, &specification)?;
        let second = issue(&mut connection, &specification)?;

        assert_eq!(first, second);
        assert_eq!(count_rows(&mut connection, "catalog_reading_rules")?, 1);
        assert_eq!(count_rows(&mut connection, "catalog_xml_repairs")?, 0);
        assert_eq!(
            count_rows(&mut connection, "catalog_file_byte_contracts")?,
            1
        );
        Ok(())
    }

    #[test]
    fn rules_can_be_issued_inside_the_imports_immediate_transaction() -> crate::Result<()> {
        let database = database()?;
        let mut connection = database.pool().get()?;
        let specification = mame_rules("nested-issue");
        connection.immediate_transaction(|connection| {
            let issued = issue(connection, &specification)?;
            assert_eq!(issue(connection, &specification)?, issued);
            Ok(())
        })
    }

    #[test]
    fn a_new_rules_key_issues_a_distinct_immutable_identity() -> crate::Result<()> {
        let database = database()?;
        let mut connection = database.pool().get()?;
        let first_specification = mame_rules("mame/0.289/observed-v2");
        let second_specification = mame_rules("mame/0.289/observed-v3");

        let first = issue(&mut connection, &first_specification)?;
        let second = issue(&mut connection, &second_specification)?;

        assert_ne!(first, second);
        assert_eq!(count_rows(&mut connection, "catalog_reading_rules")?, 2);
        Ok(())
    }

    #[test]
    fn a_rules_key_cannot_be_reused_for_a_changed_policy() -> crate::Result<()> {
        let database = database()?;
        let mut connection = database.pool().get()?;
        let original = mame_rules("mame/0.289/observed-v2");
        let original_id = issue(&mut connection, &original)?;
        let mut changed = original.clone();
        changed.rules_version = "mame-observed-compat-declared-text-v3".to_owned();

        assert!(matches!(
            issue(&mut connection, &changed),
            Err(crate::Error::DatabaseSchema(_))
        ));
        assert_eq!(issue(&mut connection, &original)?, original_id);
        assert_eq!(count_rows(&mut connection, "catalog_reading_rules")?, 1);
        assert_eq!(
            count_rows(&mut connection, "catalog_file_byte_contracts")?,
            1
        );
        Ok(())
    }

    #[test]
    fn optional_facets_are_part_of_the_immutable_rules_policy() -> crate::Result<()> {
        let database = database()?;
        let mut connection = database.pool().get()?;
        let original = mame_rules("mame/0.289/observed-v2");
        let original_id = issue(&mut connection, &original)?;
        let mut changed = original.clone();
        changed.file_byte_contract = None;

        assert!(matches!(
            issue(&mut connection, &changed),
            Err(crate::Error::DatabaseSchema(_))
        ));
        assert_eq!(issue(&mut connection, &original)?, original_id);
        assert_eq!(
            count_rows(&mut connection, "catalog_file_byte_contracts")?,
            1
        );
        Ok(())
    }

    #[test]
    fn a_byte_contract_for_another_family_is_rejected_without_partial_issue() -> crate::Result<()> {
        let database = database()?;
        let mut connection = database.pool().get()?;
        let mut specification = mame_rules("mame/0.289/observed-v2");
        specification.file_byte_contract = Some(FileByteContract::NoIntroDatUnfilteredFile);

        assert!(matches!(
            issue(&mut connection, &specification),
            Err(crate::Error::DatabaseSchema(_))
        ));
        assert_eq!(count_rows(&mut connection, "catalog_reading_rules")?, 0);
        assert_eq!(
            count_rows(&mut connection, "catalog_file_byte_contracts")?,
            0
        );
        Ok(())
    }

    #[test]
    fn xml_repair_policy_is_rejected_for_other_families() -> crate::Result<()> {
        let database = database()?;
        let mut connection = database.pool().get()?;
        let mut specification = mame_rules("mame/0.289/observed-v2");
        specification.replace_nul = Some(false);

        assert!(matches!(
            issue(&mut connection, &specification),
            Err(crate::Error::DatabaseSchema(_))
        ));
        assert_eq!(count_rows(&mut connection, "catalog_reading_rules")?, 0);
        assert_eq!(count_rows(&mut connection, "catalog_xml_repairs")?, 0);
        Ok(())
    }

    #[test]
    fn xml_repair_policy_is_reused_only_when_identical() -> crate::Result<()> {
        let database = database()?;
        let mut connection = database.pool().get()?;
        let mut original = ReadingRulesSpec {
            rules_key: "no-intro-database/nul-recovery-v2".to_owned(),
            format_family: FormatFamily::NoIntroDatabase,
            dialect: "observed-compatible".to_owned(),
            specification_version: "database-export".to_owned(),
            parser_version: "mame_coalesce-test".to_owned(),
            rules_version: "no-intro-database-nul-recovery-v2".to_owned(),
            replace_nul: Some(true),
            file_byte_contract: None,
        };
        let original_id = issue(&mut connection, &original)?;

        assert_eq!(issue(&mut connection, &original)?, original_id);
        original.replace_nul = Some(false);
        assert!(matches!(
            issue(&mut connection, &original),
            Err(crate::Error::DatabaseSchema(_))
        ));
        assert_eq!(count_rows(&mut connection, "catalog_reading_rules")?, 1);
        assert_eq!(count_rows(&mut connection, "catalog_xml_repairs")?, 1);
        Ok(())
    }
}
