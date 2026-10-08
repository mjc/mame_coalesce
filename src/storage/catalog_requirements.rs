//! Source-free reads of root-set and ROM declarations for one published edition.

use std::collections::BTreeMap;

use diesel::{
    Connection, OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use crate::{
    domain::CatalogScope,
    storage::{
        catalog_coverage::{self, CoverageId},
        catalog_hashes::HashAlgorithm,
        catalog_ids::{EditionId, MediaEntryId, ReadingRulesId, SetGroupId, SetId},
        reading_rules::{self, FormatFamily, ReadingRulesSpec},
    },
};

/// The only role returned by the root-ROM requirements reader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequirementRole {
    /// A root-set ROM/file declaration.
    Rom,
}

/// A native root-parent relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParentKind {
    /// A literal clone target.
    CloneOf,
    /// A literal ROM-set parent target.
    RomOf,
    /// A literal sample-set parent target.
    SampleOf,
    /// A flat-DAT clone target expressed by source identifier.
    CloneOfId,
    /// The bounded P/C fixture's merge-archive fallback.
    MergeOf,
}

/// A parent assertion retained from its native owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeParent {
    /// Native relationship kind.
    pub kind: ParentKind,
    /// Original literal, including an archive ID for P/C records.
    pub target_literal: String,
    /// Resolved same-group P/C target, if exactly one exists.
    pub target_set_id: Option<SetId>,
    /// Literal target name, or resolved same-group P/C name.
    pub target_name: Option<String>,
}

/// Root sets and ROM declarations for exactly one published edition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootRequirements {
    /// The exact published edition requested by the caller.
    pub edition_id: EditionId,
    /// Immutable format, parser, repair, and byte-contract facets.
    pub reading_rules: ReadingRulesSpec,
    /// Typed coverage of the exact edition.
    pub coverage: CatalogScope,
    /// Root sets in canonical source order, including sets with no ROMs.
    pub sets: Vec<RootSetRequirements>,
}

/// A root set with its native group, order, parent facts, and ROMs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootSetRequirements {
    /// Checked canonical set identifier.
    pub set_id: SetId,
    /// Checked root-group identifier.
    pub group_id: SetGroupId,
    /// Raw declared set name.
    pub name: String,
    /// Native order within the root group.
    pub order: i64,
    /// Native parent declarations; unresolved literal targets are retained.
    pub parents: Vec<NativeParent>,
    /// ROM declarations in native order.
    pub roms: Vec<RomRequirement>,
}

/// One native ROM declaration. Declared hashes are evidence, not inferred
/// whole-file expectations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RomRequirement {
    /// Checked common media-entry identifier.
    pub media_entry_id: MediaEntryId,
    /// Checked owning root set.
    pub parent_set_id: SetId,
    /// Closed requirement role.
    pub role: RequirementRole,
    /// Raw declared filename.
    pub name: String,
    /// Raw family-owned size lexeme, not a lossy numeric cast.
    pub size_text: Option<String>,
    /// Native status text, when the format owns one.
    pub status_text: Option<String>,
    /// Native merge target, when the format owns one.
    pub merge_name: Option<String>,
    /// Native order within the parent set.
    pub order: i64,
    /// Issued shared-file identity; null UUIDs do not remove declarations.
    pub file_uuid: Option<[u8; 16]>,
    /// All owned hash declarations, including empty, invalid, and non-file scopes.
    pub hashes: Vec<DeclaredHash>,
}

/// Closed source hash-field codes in the canonical schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashField {
    /// CRC spelling.
    Crc,
    /// CRC32 spelling.
    Crc32,
    /// MD5.
    Md5,
    /// SHA-1.
    Sha1,
    /// SHA-256.
    Sha256,
    /// Source-origin SHA-256, not ROM-content identity.
    OriginSha256,
}

/// Closed source-presence states for a hash declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashPresence {
    /// Present but empty.
    Empty,
    /// Present but malformed.
    Invalid,
    /// Parsed to digest bytes.
    Value,
}

/// Closed declared hash scopes. This reader never promotes scope to a
/// whole-file expected checksum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashScope {
    /// Explicit whole-file declaration.
    WholeFile,
    /// Explicit whole-asset declaration.
    WholeAsset,
    /// Scope is unknown.
    Unknown,
    /// CHD-header SHA-1.
    ChdHeaderSha1,
    /// Source-origin hash.
    SourceOrigin,
    /// NFO companion hash.
    NfoCompanion,
}

/// A valid digest value. Its declaration field and scope remain on
/// [DeclaredHash] and are not interpreted here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashValue {
    /// Stored algorithm.
    pub algorithm: HashAlgorithm,
    /// Canonical digest bytes.
    pub bytes: Vec<u8>,
}

/// One source-owned hash assertion, including unusable states.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredHash {
    /// Common reported-hash declaration ID.
    pub reported_hash_id: i64,
    /// Original source-field code.
    pub field: HashField,
    /// Native occurrence ordinal.
    pub field_occurrence: i64,
    /// Whether the source value was empty, malformed, or valid.
    pub presence: HashPresence,
    /// Declared scope, retained without checksum promotion.
    pub scope: HashScope,
    /// Original malformed lexeme, when present.
    pub reported_text: Option<String>,
    /// Interned digest value only for a valid declaration.
    pub value: Option<HashValue>,
}

#[derive(QueryableByName)]
struct PublishedEditionRow {
    #[diesel(sql_type = BigInt)]
    reading_rules_id: i64,
    #[diesel(sql_type = BigInt)]
    coverage_id: i64,
}

#[derive(QueryableByName)]
struct RootSetRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    group_id: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

#[derive(QueryableByName)]
struct RootOwnerIssue {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_element_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    owner_set_id: Option<i64>,
}

#[derive(QueryableByName)]
struct RomOwnerIssue {
    #[diesel(sql_type = BigInt)]
    source_element_id: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    owner_media_entry_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    common_media_entry_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    root_set_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    root_group_id: Option<i64>,
}

#[derive(QueryableByName)]
struct ParentRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Text)]
    parent_kind: String,
    #[diesel(sql_type = Text)]
    target_literal: String,
}

#[derive(QueryableByName)]
struct PcParentRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Text)]
    parent_kind: String,
    #[diesel(sql_type = Text)]
    target_literal: String,
    #[diesel(sql_type = Nullable<BigInt>)]
    target_set_id: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    target_name: Option<String>,
}

#[derive(QueryableByName)]
struct RomHashRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    media_entry_id: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Nullable<Text>)]
    size_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    status_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    merge_name: Option<String>,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    detail_media_entry_id: Option<i64>,
    #[diesel(sql_type = Nullable<Binary>)]
    file_uuid: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<BigInt>)]
    reported_hash_id: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    source_hash_field: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    field_occurrence: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    presence: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    hash_scope: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    reported_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    hash_algorithm: Option<String>,
    #[diesel(sql_type = Nullable<Binary>)]
    hash_bytes: Option<Vec<u8>>,
}

/// Load root sets and root-ROM declarations from exactly [edition_id].
///
/// The edition must already be published. This query is source-free: it does
/// not reopen XML, reselect a newer edition, or flatten software-list items.
pub fn load_root_requirements(
    connection: &mut SqliteConnection,
    edition_id: EditionId,
) -> crate::Result<RootRequirements> {
    connection.transaction::<_, crate::Error, _>(|connection| {
        load_published_root_requirements(connection, edition_id)
    })
}

fn load_published_root_requirements(
    connection: &mut SqliteConnection,
    edition_id: EditionId,
) -> crate::Result<RootRequirements> {
    let edition = sql_query(
        "SELECT edition.reading_rules_id, edition.coverage_id \
         FROM published_catalog_editions AS publication \
         JOIN catalog_editions AS edition ON edition.edition_id=publication.edition_id \
          AND edition.catalog_id=publication.catalog_id \
          AND edition.source_file_id=publication.source_file_id \
          AND edition.reading_rules_id=publication.reading_rules_id \
          AND edition.coverage_id=publication.coverage_id \
         WHERE publication.edition_id = ?",
    )
    .bind::<BigInt, _>(edition_id.as_i64())
    .get_result::<PublishedEditionRow>(connection)
    .optional()?
    .ok_or_else(|| {
        crate::Error::InvalidPath(format!(
            "catalog edition {} is not published",
            edition_id.as_i64()
        ))
    })?;
    let rules_id = ReadingRulesId::try_from(edition.reading_rules_id)?;
    let coverage_id = CoverageId::try_from(edition.coverage_id)?;
    let reading_rules = reading_rules::load(connection, rules_id)?;
    ensure_supported_family(reading_rules.format_family)?;
    let coverage = catalog_coverage::load(connection, coverage_id)?;
    validate_root_owner_ancestry(connection, edition_id, reading_rules.format_family)?;
    validate_rom_owner_ancestry(connection, edition_id, reading_rules.format_family)?;

    let set_query = with_publication(
        "SELECT sets.set_id, groups.set_group_id AS group_id, sets.set_name AS name, \
                sets.source_order \
         FROM selected_publication AS publication \
         JOIN catalog_set_groups AS groups ON groups.edition_id=publication.edition_id \
         JOIN catalog_sets AS sets USING (set_group_id) \
         WHERE groups.group_kind = 'root' \
         ORDER BY sets.source_order, sets.set_id",
    );
    let mut sets = sql_query(set_query)
        .bind::<BigInt, _>(edition_id.as_i64())
        .load::<RootSetRow>(connection)?
        .into_iter()
        .map(|row| {
            Ok(RootSetRequirements {
                set_id: SetId::try_from(row.set_id)?,
                group_id: SetGroupId::try_from(row.group_id)?,
                name: row.name,
                order: row.source_order,
                parents: Vec::new(),
                roms: Vec::new(),
            })
        })
        .collect::<crate::Result<Vec<_>>>()?;
    let mut names = std::collections::BTreeSet::new();
    for set in &sets {
        if !names.insert(set.name.as_str()) {
            return Err(crate::Error::InvalidPath(format!(
                "published edition {} repeats root set name {:?}; build layout requires unique names",
                edition_id.as_i64(),
                set.name
            )));
        }
    }
    let indexes = sets
        .iter()
        .enumerate()
        .map(|(index, set)| (set.set_id, index))
        .collect::<BTreeMap<_, _>>();

    load_native_parents(
        connection,
        edition_id,
        reading_rules.format_family,
        &mut sets,
        &indexes,
    )?;
    load_native_roms(
        connection,
        edition_id,
        reading_rules.format_family,
        &mut sets,
        &indexes,
    )?;
    Ok(RootRequirements {
        edition_id,
        reading_rules,
        coverage,
        sets,
    })
}

const SELECTED_PUBLICATION_CTE: &str = "WITH selected_publication AS ( \
    SELECT publication.edition_id, publication.catalog_id, publication.source_file_id, \
           publication.reading_rules_id, publication.coverage_id \
    FROM published_catalog_editions AS publication \
    JOIN catalog_editions AS edition ON edition.edition_id=publication.edition_id \
      AND edition.catalog_id=publication.catalog_id \
      AND edition.source_file_id=publication.source_file_id \
      AND edition.reading_rules_id=publication.reading_rules_id \
      AND edition.coverage_id=publication.coverage_id \
    WHERE publication.edition_id=? \
)";

fn with_publication(query: &str) -> String {
    format!("{SELECTED_PUBLICATION_CTE} {query}")
}

fn ensure_supported_family(family: FormatFamily) -> crate::Result<()> {
    match family {
        FormatFamily::Mame
        | FormatFamily::Logiqx
        | FormatFamily::ClrMamePro
        | FormatFamily::NoIntroDat
        | FormatFamily::NoIntroPcFixture => Ok(()),
        FormatFamily::Software => Err(unsupported_family(
            family,
            "software-list items are not root ROM requirements",
        )),
        FormatFamily::NoIntroDatabase => Err(unsupported_family(
            family,
            "database-export source-file facts are not root ROM requirements",
        )),
    }
}

fn unsupported_family(family: FormatFamily, reason: &str) -> crate::Error {
    crate::Error::InvalidPath(format!("catalog family {family:?}: {reason}"))
}

struct FamilyTables {
    root_table: &'static str,
    root_element_kind: &'static str,
    rom_table: &'static str,
    rom_parent_column: &'static str,
    rom_element_kind: &'static str,
}

fn family_tables(family: FormatFamily) -> crate::Result<FamilyTables> {
    match family {
        FormatFamily::Mame => Ok(FamilyTables {
            root_table: "mame_machines",
            root_element_kind: "mame_machine",
            rom_table: "mame_roms",
            rom_parent_column: "machine_id",
            rom_element_kind: "mame_rom",
        }),
        FormatFamily::Logiqx => Ok(FamilyTables {
            root_table: "logiqx_games",
            root_element_kind: "logiqx_game",
            rom_table: "logiqx_roms",
            rom_parent_column: "set_id",
            rom_element_kind: "logiqx_rom",
        }),
        FormatFamily::ClrMamePro => Ok(FamilyTables {
            root_table: "clrmamepro_sets",
            root_element_kind: "clrmamepro_set",
            rom_table: "clrmamepro_roms",
            rom_parent_column: "set_id",
            rom_element_kind: "clrmamepro_rom",
        }),
        FormatFamily::NoIntroDat => Ok(FamilyTables {
            root_table: "no_intro_dat_games",
            root_element_kind: "no_intro_dat_game",
            rom_table: "no_intro_dat_rom_claims",
            rom_parent_column: "set_id",
            rom_element_kind: "no_intro_dat_rom",
        }),
        FormatFamily::NoIntroPcFixture => Ok(FamilyTables {
            root_table: "no_intro_pc_games",
            root_element_kind: "no_intro_pc_game",
            rom_table: "no_intro_pc_file_claims",
            rom_parent_column: "set_id",
            rom_element_kind: "no_intro_pc_rom",
        }),
        FormatFamily::Software | FormatFamily::NoIntroDatabase => Err(unsupported_family(
            family,
            "family has no root ROM owner mapping",
        )),
    }
}

fn validate_root_owner_ancestry(
    connection: &mut SqliteConnection,
    edition_id: EditionId,
    family: FormatFamily,
) -> crate::Result<()> {
    let tables = family_tables(family)?;
    let query = with_publication(&format!(
        "SELECT sets.set_id, source.source_element_id, owner.set_id AS owner_set_id \
         FROM selected_publication AS publication \
         JOIN catalog_set_groups AS groups ON groups.edition_id=publication.edition_id \
         JOIN catalog_sets AS sets USING (set_group_id) \
         LEFT JOIN catalog_source_elements AS source \
           ON source.source_element_id=sets.set_id \
          AND source.edition_id=publication.edition_id \
          AND source.element_kind='{}' \
         LEFT JOIN {} AS owner ON owner.set_id=sets.set_id \
         WHERE groups.group_kind='root' \
           AND (source.source_element_id IS NULL OR owner.set_id IS NULL) \
         ORDER BY sets.source_order, sets.set_id LIMIT 1",
        tables.root_element_kind, tables.root_table
    ));
    if let Some(issue) = sql_query(query)
        .bind::<BigInt, _>(edition_id.as_i64())
        .get_result::<RootOwnerIssue>(connection)
        .optional()?
    {
        return Err(crate::Error::DatabaseSchema(format!(
            "root set {} has missing or wrong {} source-owner ancestry (source {:?}, owner {:?})",
            issue.set_id, tables.root_table, issue.source_element_id, issue.owner_set_id
        )));
    }
    Ok(())
}

fn validate_rom_owner_ancestry(
    connection: &mut SqliteConnection,
    edition_id: EditionId,
    family: FormatFamily,
) -> crate::Result<()> {
    let tables = family_tables(family)?;
    let missing_owner_query = with_publication(&format!(
        "SELECT source.source_element_id, owner.media_entry_id AS owner_media_entry_id, \
                media.media_entry_id AS common_media_entry_id, root.set_id AS root_set_id, \
                root_group.set_group_id AS root_group_id \
         FROM selected_publication AS publication \
         JOIN catalog_source_elements AS source \
           ON source.edition_id=publication.edition_id AND source.element_kind='{}' \
         LEFT JOIN {} AS owner ON owner.media_entry_id=source.source_element_id \
         LEFT JOIN catalog_media_entries AS media ON media.media_entry_id=owner.media_entry_id \
         LEFT JOIN {} AS root_owner ON root_owner.set_id=owner.{} \
         LEFT JOIN catalog_sets AS root ON root.set_id=root_owner.set_id \
         LEFT JOIN catalog_set_groups AS root_group \
           ON root_group.set_group_id=root.set_group_id \
          AND root_group.edition_id=publication.edition_id AND root_group.group_kind='root' \
         WHERE owner.media_entry_id IS NULL OR media.media_entry_id IS NULL \
            OR root.set_id IS NULL OR root_group.set_group_id IS NULL \
         ORDER BY source.source_element_id LIMIT 1",
        tables.rom_element_kind, tables.rom_table, tables.root_table, tables.rom_parent_column
    ));
    if let Some(issue) = sql_query(missing_owner_query)
        .bind::<BigInt, _>(edition_id.as_i64())
        .get_result::<RomOwnerIssue>(connection)
        .optional()?
    {
        return Err(rom_ancestry_error(&tables, issue));
    }

    let wrong_source_query = with_publication(&format!(
        "SELECT owner.media_entry_id AS source_element_id, owner.media_entry_id AS owner_media_entry_id, \
                media.media_entry_id AS common_media_entry_id, root.set_id AS root_set_id, \
                root_group.set_group_id AS root_group_id \
         FROM selected_publication AS publication \
         JOIN catalog_set_groups AS root_group ON root_group.edition_id=publication.edition_id \
            AND root_group.group_kind='root' \
         JOIN catalog_sets AS root USING (set_group_id) \
         JOIN {} AS root_owner ON root_owner.set_id=root.set_id \
         JOIN {} AS owner ON owner.{}=root_owner.set_id \
         LEFT JOIN catalog_source_elements AS source \
           ON source.source_element_id=owner.media_entry_id \
          AND source.edition_id=publication.edition_id AND source.element_kind='{}' \
         LEFT JOIN catalog_media_entries AS media ON media.media_entry_id=owner.media_entry_id \
         WHERE source.source_element_id IS NULL OR media.media_entry_id IS NULL \
         ORDER BY owner.media_entry_id LIMIT 1",
        tables.root_table, tables.rom_table, tables.rom_parent_column, tables.rom_element_kind
    ));
    if let Some(issue) = sql_query(wrong_source_query)
        .bind::<BigInt, _>(edition_id.as_i64())
        .get_result::<RomOwnerIssue>(connection)
        .optional()?
    {
        return Err(rom_ancestry_error(&tables, issue));
    }
    Ok(())
}

fn rom_ancestry_error(tables: &FamilyTables, issue: RomOwnerIssue) -> crate::Error {
    crate::Error::DatabaseSchema(format!(
        "ROM source element {} has missing or wrong {} ancestry (owner {:?}, media {:?}, root set {:?}, root group {:?})",
        issue.source_element_id,
        tables.rom_table,
        issue.owner_media_entry_id,
        issue.common_media_entry_id,
        issue.root_set_id,
        issue.root_group_id
    ))
}

fn load_native_parents(
    connection: &mut SqliteConnection,
    edition_id: EditionId,
    family: FormatFamily,
    sets: &mut [RootSetRequirements],
    indexes: &BTreeMap<SetId, usize>,
) -> crate::Result<()> {
    match family {
        FormatFamily::Mame => load_parent_rows(
            connection,
            edition_id,
            "SELECT machine.set_id, links.link_kind AS parent_kind, links.target_name AS target_literal \
             FROM selected_publication AS publication \
             JOIN catalog_set_groups AS groups ON groups.edition_id=publication.edition_id \
             JOIN catalog_sets AS sets USING (set_group_id) \
             JOIN mame_machines AS machine USING (set_id) \
             JOIN mame_machine_links AS links ON links.machine_id = machine.set_id \
             WHERE groups.group_kind = 'root' \
             ORDER BY machine.set_id, links.link_kind",
            sets,
            indexes,
        ),
        FormatFamily::Logiqx => load_parent_rows(
            connection,
            edition_id,
            "SELECT game.set_id, links.link_kind AS parent_kind, links.target_name AS target_literal \
             FROM selected_publication AS publication \
             JOIN catalog_set_groups AS groups ON groups.edition_id=publication.edition_id \
             JOIN catalog_sets AS sets USING (set_group_id) \
             JOIN logiqx_games AS game USING (set_id) \
             JOIN logiqx_set_links AS links USING (set_id) \
             WHERE groups.group_kind = 'root' \
             ORDER BY game.set_id, links.link_kind",
            sets,
            indexes,
        ),
        FormatFamily::ClrMamePro => load_parent_rows(
            connection,
            edition_id,
            "SELECT owner.set_id, links.link_kind AS parent_kind, links.target_name AS target_literal \
             FROM selected_publication AS publication \
             JOIN catalog_set_groups AS groups ON groups.edition_id=publication.edition_id \
             JOIN catalog_sets AS sets USING (set_group_id) \
             JOIN clrmamepro_sets AS owner USING (set_id) \
             JOIN clrmamepro_set_links AS links USING (set_id) \
             WHERE groups.group_kind = 'root' \
             ORDER BY owner.set_id, links.link_kind",
            sets,
            indexes,
        ),
        FormatFamily::NoIntroDat => load_parent_rows(
            connection,
            edition_id,
            "SELECT owner.set_id, links.link_kind AS parent_kind, links.target_literal \
             FROM selected_publication AS publication \
             JOIN catalog_set_groups AS groups ON groups.edition_id=publication.edition_id \
             JOIN catalog_sets AS sets USING (set_group_id) \
             JOIN no_intro_dat_games AS owner USING (set_id) \
             JOIN no_intro_dat_set_links AS links USING (set_id) \
             WHERE groups.group_kind = 'root' \
             ORDER BY owner.set_id, links.link_kind",
            sets,
            indexes,
        ),
        FormatFamily::NoIntroPcFixture => load_pc_parents(connection, edition_id, sets, indexes),
        FormatFamily::Software | FormatFamily::NoIntroDatabase => Err(unsupported_family(
            family,
            "family has no supported root reader",
        )),
    }
}

fn load_parent_rows(
    connection: &mut SqliteConnection,
    edition_id: EditionId,
    query: &str,
    sets: &mut [RootSetRequirements],
    indexes: &BTreeMap<SetId, usize>,
) -> crate::Result<()> {
    for row in sql_query(with_publication(query))
        .bind::<BigInt, _>(edition_id.as_i64())
        .load::<ParentRow>(connection)?
    {
        let set_id = SetId::try_from(row.set_id)?;
        let index = root_set_index(indexes, set_id)?;
        let kind = parse_parent_kind(&row.parent_kind)?;
        sets[index].parents.push(NativeParent {
            kind,
            target_literal: row.target_literal.clone(),
            target_set_id: None,
            target_name: Some(row.target_literal),
        });
    }
    Ok(())
}

fn load_pc_parents(
    connection: &mut SqliteConnection,
    edition_id: EditionId,
    sets: &mut [RootSetRequirements],
    indexes: &BTreeMap<SetId, usize>,
) -> crate::Result<()> {
    let rows = sql_query(with_publication(
        "SELECT child.set_id, links.parent_kind, links.target_literal, \
                target.set_id AS target_set_id, target.set_name AS target_name \
         FROM selected_publication AS publication \
         JOIN catalog_set_groups AS groups ON groups.edition_id=publication.edition_id \
         JOIN catalog_sets AS child USING (set_group_id) \
         JOIN ( \
             SELECT clone.set_id, 'cloneof' AS parent_kind, clone.target_archive_id AS target_literal \
             FROM no_intro_pc_clone_links AS clone \
             UNION ALL \
             SELECT merge.set_id, 'mergeof' AS parent_kind, merge.target_archive_id AS target_literal \
             FROM no_intro_pc_merge_links AS merge \
             WHERE NOT EXISTS (SELECT 1 FROM no_intro_pc_clone_links AS clone WHERE clone.set_id=merge.set_id) \
         ) AS links ON links.set_id = child.set_id \
         LEFT JOIN no_intro_pc_games AS target_game ON target_game.archive_id = links.target_literal \
             AND target_game.set_id IN (SELECT candidate.set_id FROM catalog_sets AS candidate \
                 WHERE candidate.set_group_id = groups.set_group_id) \
         LEFT JOIN catalog_sets AS target ON target.set_id = target_game.set_id \
             AND target.set_group_id = groups.set_group_id \
         WHERE groups.group_kind = 'root' \
         ORDER BY child.set_id, target.set_id",
    ))
    .bind::<BigInt, _>(edition_id.as_i64())
    .load::<PcParentRow>(connection)?;
    let mut targets = BTreeMap::<(SetId, ParentKind), Option<SetId>>::new();
    for row in rows {
        let set_id = SetId::try_from(row.set_id)?;
        let index = root_set_index(indexes, set_id)?;
        let kind = parse_parent_kind(&row.parent_kind)?;
        let target_set_id = row.target_set_id.map(SetId::try_from).transpose()?;
        if targets.insert((set_id, kind), target_set_id).is_some() {
            return Err(crate::Error::InvalidPath(format!(
                "P/C parent archive ID {:?} is ambiguous in root set {}'s group",
                row.target_literal,
                set_id.as_i64()
            )));
        }
        sets[index].parents.push(NativeParent {
            kind,
            target_literal: row.target_literal,
            target_set_id,
            target_name: row.target_name,
        });
    }
    Ok(())
}

fn parse_parent_kind(value: &str) -> crate::Result<ParentKind> {
    match value {
        "cloneof" => Ok(ParentKind::CloneOf),
        "romof" => Ok(ParentKind::RomOf),
        "sampleof" => Ok(ParentKind::SampleOf),
        "cloneofid" => Ok(ParentKind::CloneOfId),
        "mergeof" => Ok(ParentKind::MergeOf),
        _ => Err(invalid_value("root parent kind", value)),
    }
}

fn load_native_roms(
    connection: &mut SqliteConnection,
    edition_id: EditionId,
    family: FormatFamily,
    sets: &mut [RootSetRequirements],
    indexes: &BTreeMap<SetId, usize>,
) -> crate::Result<()> {
    let query = match family {
        FormatFamily::Mame => MAME_ROMS,
        FormatFamily::Logiqx => LOGIQX_ROMS,
        FormatFamily::ClrMamePro => CMP_ROMS,
        FormatFamily::NoIntroDat => NO_INTRO_DAT_ROMS,
        FormatFamily::NoIntroPcFixture => NO_INTRO_PC_ROMS,
        FormatFamily::Software | FormatFamily::NoIntroDatabase => {
            return Err(unsupported_family(
                family,
                "family has no supported root ROM reader",
            ));
        }
    };
    let mut roms = BTreeMap::<MediaEntryId, RomRequirement>::new();
    for row in sql_query(with_publication(query))
        .bind::<BigInt, _>(edition_id.as_i64())
        .load::<RomHashRow>(connection)?
    {
        let set_id = SetId::try_from(row.set_id)?;
        let set_index = root_set_index(indexes, set_id)?;
        let media_entry_id = MediaEntryId::try_from(row.media_entry_id)?;
        if family == FormatFamily::ClrMamePro
            && row.detail_media_entry_id != Some(media_entry_id.as_i64())
        {
            return Err(crate::Error::DatabaseSchema(format!(
                "CMP ROM {} is missing its required detail owner",
                media_entry_id.as_i64()
            )));
        }
        let file_uuid = row
            .file_uuid
            .as_deref()
            .map(|bytes| {
                bytes.try_into().map_err(|_| {
                    crate::Error::DatabaseSchema(format!(
                        "media entry {} has a {}-byte UUID; expected 16",
                        media_entry_id.as_i64(),
                        bytes.len()
                    ))
                })
            })
            .transpose()?;
        let rom = roms
            .entry(media_entry_id)
            .or_insert_with(|| RomRequirement {
                media_entry_id,
                parent_set_id: set_id,
                role: RequirementRole::Rom,
                name: row.name.clone(),
                size_text: row.size_text.clone(),
                status_text: row.status_text.clone(),
                merge_name: row.merge_name.clone(),
                order: row.source_order,
                file_uuid,
                hashes: Vec::new(),
            });
        if rom.parent_set_id != sets[set_index].set_id
            || rom.name != row.name
            || rom.order != row.source_order
        {
            return Err(crate::Error::DatabaseSchema(format!(
                "media entry {} has inconsistent joined root ownership",
                media_entry_id.as_i64()
            )));
        }
        if let Some(hash) = decode_hash(&row)? {
            rom.hashes.push(hash);
        }
    }
    for rom in roms.into_values() {
        let index = root_set_index(indexes, rom.parent_set_id)?;
        sets[index].roms.push(rom);
    }
    for set in sets {
        set.roms.sort_by_key(|rom| (rom.order, rom.media_entry_id));
    }
    Ok(())
}

fn required_hash_text<'a>(value: &'a Option<String>, column: &str) -> crate::Result<&'a str> {
    value
        .as_deref()
        .ok_or_else(|| invalid_value(column, "NULL on a hash declaration"))
}

fn decode_hash(row: &RomHashRow) -> crate::Result<Option<DeclaredHash>> {
    let Some(reported_hash_id) = row.reported_hash_id else {
        return Ok(None);
    };
    let field = parse_hash_field(required_hash_text(
        &row.source_hash_field,
        "source hash field",
    )?)?;
    let presence = parse_hash_presence(required_hash_text(&row.presence, "hash presence")?)?;
    let scope = parse_hash_scope(required_hash_text(&row.hash_scope, "hash scope")?)?;
    let field_occurrence = row
        .field_occurrence
        .ok_or_else(|| invalid_value("hash field occurrence", "NULL on a hash declaration"))?;
    if field_occurrence < 0 {
        return Err(invalid_value(
            "hash field occurrence",
            &field_occurrence.to_string(),
        ));
    }
    let value = match (row.hash_algorithm.as_deref(), row.hash_bytes.as_ref()) {
        (None, None) => None,
        (Some(algorithm), Some(bytes)) => {
            let algorithm = HashAlgorithm::from_database(algorithm)?;
            if bytes.len() != algorithm.byte_length() {
                return Err(invalid_value(
                    "hash_values.bytes",
                    "digest length does not match its algorithm",
                ));
            }
            Some(HashValue {
                algorithm,
                bytes: bytes.clone(),
            })
        }
        _ => return Err(invalid_value("hash value", "incomplete interned digest")),
    };
    if (presence == HashPresence::Value) != value.is_some() {
        return Err(invalid_value(
            "hash presence/value",
            "stored state does not match its interned digest",
        ));
    }
    Ok(Some(DeclaredHash {
        reported_hash_id,
        field,
        field_occurrence,
        presence,
        scope,
        reported_text: row.reported_text.clone(),
        value,
    }))
}

fn root_set_index(indexes: &BTreeMap<SetId, usize>, set_id: SetId) -> crate::Result<usize> {
    indexes.get(&set_id).copied().ok_or_else(|| {
        crate::Error::DatabaseSchema(format!(
            "native root owner references set {} outside its published root group",
            set_id.as_i64()
        ))
    })
}

fn parse_hash_field(value: &str) -> crate::Result<HashField> {
    match value {
        "crc" => Ok(HashField::Crc),
        "crc32" => Ok(HashField::Crc32),
        "md5" => Ok(HashField::Md5),
        "sha1" => Ok(HashField::Sha1),
        "sha256" => Ok(HashField::Sha256),
        "origin_sha256" => Ok(HashField::OriginSha256),
        _ => Err(invalid_value(
            "catalog_entry_hashes.source_hash_field",
            value,
        )),
    }
}

fn parse_hash_presence(value: &str) -> crate::Result<HashPresence> {
    match value {
        "empty" => Ok(HashPresence::Empty),
        "invalid" => Ok(HashPresence::Invalid),
        "value" => Ok(HashPresence::Value),
        _ => Err(invalid_value("catalog_entry_hashes.presence", value)),
    }
}

fn parse_hash_scope(value: &str) -> crate::Result<HashScope> {
    match value {
        "whole_file" => Ok(HashScope::WholeFile),
        "whole_asset" => Ok(HashScope::WholeAsset),
        "unknown" => Ok(HashScope::Unknown),
        "chd_header_sha1" => Ok(HashScope::ChdHeaderSha1),
        "source_origin" => Ok(HashScope::SourceOrigin),
        "nfo_companion" => Ok(HashScope::NfoCompanion),
        _ => Err(invalid_value("catalog_entry_hashes.hash_scope", value)),
    }
}

fn invalid_value(field: &str, value: &str) -> crate::Error {
    crate::Error::DatabaseSchema(format!("invalid persisted {field}: {value}"))
}

const MAME_ROMS: &str = "SELECT owner.machine_id AS set_id, owner.media_entry_id, owner.name, \
    owner.size_text, owner.status AS status_text, merge.merge_name, owner.source_order, \
    owner.media_entry_id AS detail_media_entry_id, media.file_uuid, hash.reported_hash_id, \
    hash.source_hash_field, hash.field_occurrence, \
    hash.presence, hash.hash_scope, hash.reported_text, value.algorithm AS hash_algorithm, \
    value.bytes AS hash_bytes \
    FROM selected_publication AS publication \
    JOIN catalog_set_groups AS groups ON groups.edition_id=publication.edition_id \
    JOIN catalog_sets AS sets USING (set_group_id) \
    JOIN mame_machines AS machine USING (set_id) \
    JOIN mame_roms AS owner ON owner.machine_id=machine.set_id \
    JOIN catalog_source_elements AS element ON element.source_element_id=owner.media_entry_id \
        AND element.edition_id=publication.edition_id AND element.element_kind='mame_rom' \
    JOIN catalog_media_entries AS media ON media.media_entry_id=owner.media_entry_id \
    LEFT JOIN mame_rom_merges AS merge ON merge.media_entry_id=owner.media_entry_id \
    LEFT JOIN catalog_entry_hashes AS hash ON hash.media_entry_id=owner.media_entry_id \
    LEFT JOIN hash_values AS value ON value.hash_id=hash.hash_id \
    WHERE groups.group_kind='root' \
    ORDER BY sets.source_order,sets.set_id,owner.source_order,owner.media_entry_id,hash.reported_hash_id";

const LOGIQX_ROMS: &str = "SELECT owner.set_id, owner.media_entry_id, owner.name, owner.size_text, \
    owner.status AS status_text, merge.merge_name, owner.source_order, \
    owner.media_entry_id AS detail_media_entry_id, media.file_uuid, \
    hash.reported_hash_id, hash.source_hash_field, hash.field_occurrence, hash.presence, \
    hash.hash_scope, hash.reported_text, value.algorithm AS hash_algorithm, value.bytes AS hash_bytes \
    FROM selected_publication AS publication \
    JOIN catalog_set_groups AS groups ON groups.edition_id=publication.edition_id \
    JOIN catalog_sets AS sets USING (set_group_id) \
    JOIN logiqx_games AS game USING (set_id) \
    JOIN logiqx_roms AS owner USING (set_id) \
    JOIN catalog_source_elements AS element ON element.source_element_id=owner.media_entry_id \
        AND element.edition_id=publication.edition_id AND element.element_kind='logiqx_rom' \
    JOIN catalog_media_entries AS media ON media.media_entry_id=owner.media_entry_id \
    LEFT JOIN logiqx_rom_merges AS merge ON merge.media_entry_id=owner.media_entry_id \
    LEFT JOIN catalog_entry_hashes AS hash ON hash.media_entry_id=owner.media_entry_id \
    LEFT JOIN hash_values AS value ON value.hash_id=hash.hash_id \
    WHERE groups.group_kind='root' \
    ORDER BY sets.source_order,sets.set_id,owner.source_order,owner.media_entry_id,hash.reported_hash_id";

const CMP_ROMS: &str = "SELECT owner.set_id, owner.media_entry_id, owner.name, owner.size_text, \
    details.status_text, merge.merge_name, owner.source_order, \
    details.media_entry_id AS detail_media_entry_id, media.file_uuid, \
    hash.reported_hash_id, hash.source_hash_field, hash.field_occurrence, hash.presence, \
    hash.hash_scope, hash.reported_text, value.algorithm AS hash_algorithm, value.bytes AS hash_bytes \
    FROM selected_publication AS publication \
    JOIN catalog_set_groups AS groups ON groups.edition_id=publication.edition_id \
    JOIN catalog_sets AS sets USING (set_group_id) \
    JOIN clrmamepro_sets AS game USING (set_id) \
    JOIN clrmamepro_roms AS owner USING (set_id) \
    JOIN catalog_source_elements AS element ON element.source_element_id=owner.media_entry_id \
        AND element.edition_id=publication.edition_id AND element.element_kind='clrmamepro_rom' \
    LEFT JOIN clrmamepro_rom_details AS details USING (media_entry_id) \
    JOIN catalog_media_entries AS media ON media.media_entry_id=owner.media_entry_id \
    LEFT JOIN clrmamepro_rom_merges AS merge ON merge.media_entry_id=owner.media_entry_id \
    LEFT JOIN catalog_entry_hashes AS hash ON hash.media_entry_id=owner.media_entry_id \
    LEFT JOIN hash_values AS value ON value.hash_id=hash.hash_id \
    WHERE groups.group_kind='root' \
    ORDER BY sets.source_order,sets.set_id,owner.source_order,owner.media_entry_id,hash.reported_hash_id";

const NO_INTRO_DAT_ROMS: &str = "SELECT owner.set_id, owner.media_entry_id, owner.name, owner.size_text, \
    owner.status_text, NULL AS merge_name, owner.source_order, \
    owner.media_entry_id AS detail_media_entry_id, media.file_uuid, \
    hash.reported_hash_id, hash.source_hash_field, hash.field_occurrence, hash.presence, \
    hash.hash_scope, hash.reported_text, value.algorithm AS hash_algorithm, value.bytes AS hash_bytes \
    FROM selected_publication AS publication \
    JOIN catalog_set_groups AS groups ON groups.edition_id=publication.edition_id \
    JOIN catalog_sets AS sets USING (set_group_id) \
    JOIN no_intro_dat_games AS game USING (set_id) \
    JOIN no_intro_dat_rom_claims AS owner USING (set_id) \
    JOIN catalog_source_elements AS element ON element.source_element_id=owner.media_entry_id \
        AND element.edition_id=publication.edition_id AND element.element_kind='no_intro_dat_rom' \
    JOIN catalog_media_entries AS media ON media.media_entry_id=owner.media_entry_id \
    LEFT JOIN catalog_entry_hashes AS hash ON hash.media_entry_id=owner.media_entry_id \
    LEFT JOIN hash_values AS value ON value.hash_id=hash.hash_id \
    WHERE groups.group_kind='root' \
    ORDER BY sets.source_order,sets.set_id,owner.source_order,owner.media_entry_id,hash.reported_hash_id";

const NO_INTRO_PC_ROMS: &str = "SELECT owner.set_id, owner.media_entry_id, owner.name, owner.size_text, \
    NULL AS status_text, NULL AS merge_name, owner.source_order, \
    owner.media_entry_id AS detail_media_entry_id, media.file_uuid, \
    hash.reported_hash_id, hash.source_hash_field, hash.field_occurrence, hash.presence, \
    hash.hash_scope, hash.reported_text, value.algorithm AS hash_algorithm, value.bytes AS hash_bytes \
    FROM selected_publication AS publication \
    JOIN catalog_set_groups AS groups ON groups.edition_id=publication.edition_id \
    JOIN catalog_sets AS sets USING (set_group_id) \
    JOIN no_intro_pc_games AS game USING (set_id) \
    JOIN no_intro_pc_file_claims AS owner USING (set_id) \
    JOIN catalog_source_elements AS element ON element.source_element_id=owner.media_entry_id \
        AND element.edition_id=publication.edition_id AND element.element_kind='no_intro_pc_rom' \
    JOIN catalog_media_entries AS media ON media.media_entry_id=owner.media_entry_id \
    LEFT JOIN catalog_entry_hashes AS hash ON hash.media_entry_id=owner.media_entry_id \
    LEFT JOIN hash_values AS value ON value.hash_id=hash.hash_id \
    WHERE groups.group_kind='root' \
    ORDER BY sets.source_order,sets.set_id,owner.source_order,owner.media_entry_id,hash.reported_hash_id";

#[cfg(test)]
mod tests;
