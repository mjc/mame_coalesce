use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use crate::{
    domain::SnapshotKey,
    no_intro_dat_xml::{DeclaredText, Document, Game, Release, Rom},
    storage::{
        catalog_content::{
            ContentDigestAssertions, ContentIdentityResolution, record_content_identity_conflict,
            record_occurrence_digest_assertions, resolve_content_identity,
        },
        catalog_identity::OccurrenceId,
    },
};

const MODES: [&str; 4] = [
    "no-intro-dat-v3-strict",
    "no-intro-dat-v3-compatible",
    "no-intro-dat-v4-strict",
    "no-intro-dat-v4-compatible",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DigestScope {
    WholeFile,
    Unknown,
}

impl DigestScope {
    #[must_use]
    pub(super) fn from_document(document: &Document) -> Self {
        if document
            .header
            .clrmamepro
            .as_ref()
            .is_some_and(|options| options.header.is_some())
        {
            Self::Unknown
        } else {
            Self::WholeFile
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::WholeFile => "whole_file",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy)]
#[repr(i64)]
enum HeaderField {
    Id = 0,
    Name = 1,
    Description = 2,
    Version = 3,
    Date = 4,
    Author = 5,
    Homepage = 6,
    Url = 7,
    Trademarks = 8,
    Piracy = 9,
    Subset = 10,
    Comment = 11,
}

#[derive(Clone, Copy)]
#[repr(i64)]
enum ClrMameProField {
    ForceNoDump = 0,
    Header = 1,
}

#[derive(Clone, Copy)]
#[repr(i64)]
enum RomCenterField {
    Plugin = 0,
}

#[derive(Clone, Copy)]
#[repr(i64)]
enum GameField {
    Name = 0,
    Id = 1,
    CloneOf = 2,
    CloneOfId = 3,
    Description = 4,
}

#[derive(Clone, Copy)]
#[repr(i64)]
enum RomField {
    Name = 0,
    Size = 1,
    Crc = 2,
    Md5 = 3,
    Sha1 = 4,
    Sha256 = 5,
    Status = 6,
    Serial = 7,
    Header = 8,
    Date = 9,
    Mia = 10,
}

#[derive(Clone, Copy)]
enum RomDigestField {
    Crc,
    Md5,
    Sha1,
    Sha256,
}

impl RomDigestField {
    const fn position_kind(self) -> RomField {
        match self {
            Self::Crc => RomField::Crc,
            Self::Md5 => RomField::Md5,
            Self::Sha1 => RomField::Sha1,
            Self::Sha256 => RomField::Sha256,
        }
    }

    const fn algorithm(self) -> &'static str {
        match self {
            Self::Crc => "crc32",
            Self::Md5 => "md5",
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
        }
    }
}

#[derive(Clone, Copy)]
enum DeclaredDigest<'a> {
    Usable(&'a [u8]),
    Invalid(&'a str),
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ImportCounts {
    games: i64,
    roms: i64,
    categories: i64,
    identifiers: i64,
    releases: i64,
    clrmamepro_options: i64,
    romcenter_options: i64,
    header_fields: i64,
    clrmamepro_fields: i64,
    romcenter_fields: i64,
    game_fields: i64,
    rom_fields: i64,
}

impl ImportCounts {
    #[must_use]
    pub(super) fn from_document(document: &Document) -> Self {
        let header = &document.header;
        Self {
            header_fields: present_count([
                header.id.is_some(),
                header.name.is_some(),
                header.description.is_some(),
                header.version.is_some(),
                header.date.is_some(),
                header.author.is_some(),
                header.homepage.is_some(),
                header.url.is_some(),
                header.trademarks.is_some(),
                header.piracy.is_some(),
                header.subset.is_some(),
                header.comment.is_some(),
            ]),
            clrmamepro_fields: header.clrmamepro.as_ref().map_or(0, |options| {
                present_count([options.forcenodump.is_some(), options.header.is_some()])
            }),
            clrmamepro_options: i64::from(header.clrmamepro.is_some()),
            romcenter_fields: header
                .romcenter
                .as_ref()
                .map_or(0, |options| i64::from(options.plugin.is_some())),
            romcenter_options: i64::from(header.romcenter.is_some()),
            ..Self::default()
        }
    }

    pub(super) fn include_game(&mut self, game: &Game) -> crate::Result<()> {
        checked_add(&mut self.games, 1, "No-Intro DAT games")?;
        checked_add(
            &mut self.roms,
            checked_len(game.roms.len(), "No-Intro DAT ROMs")?,
            "No-Intro DAT ROMs",
        )?;
        checked_add(
            &mut self.categories,
            checked_len(game.categories.len(), "No-Intro DAT categories")?,
            "No-Intro DAT categories",
        )?;
        checked_add(
            &mut self.identifiers,
            checked_len(game.identifiers.len(), "No-Intro DAT identifiers")?,
            "No-Intro DAT identifiers",
        )?;
        checked_add(
            &mut self.releases,
            checked_len(game.releases.len(), "No-Intro DAT releases")?,
            "No-Intro DAT releases",
        )?;
        checked_add(
            &mut self.game_fields,
            present_count([
                true,
                game.id.is_some(),
                game.cloneof.is_some(),
                game.cloneofid.is_some(),
                game.description.is_some(),
            ]),
            "No-Intro DAT game fields",
        )?;
        for rom in &game.roms {
            checked_add(
                &mut self.rom_fields,
                present_count([
                    true,
                    rom.size.is_some(),
                    rom.crc.is_some(),
                    rom.md5.is_some(),
                    rom.sha1.is_some(),
                    rom.sha256.is_some(),
                    rom.status.is_some(),
                    rom.serial.is_some(),
                    rom.header.is_some(),
                    rom.date.is_some(),
                    rom.mia.is_some(),
                ]),
                "No-Intro DAT ROM fields",
            )?;
        }
        Ok(())
    }
}

pub(super) fn insert_document(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    document: &Document,
) -> crate::Result<()> {
    require_mode(conn, snapshot_key)?;
    sql_query(
        "INSERT INTO no_intro_dat_documents(snapshot_key,schema_location,source_line,source_column) \
         VALUES (?,?,?,?)",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<Nullable<Text>, _>(document.schema_location.as_deref())
    .bind::<BigInt, _>(document.location.line)
    .bind::<BigInt, _>(document.location.column)
    .execute(conn)?;

    let header = &document.header;
    sql_query(
        "INSERT INTO no_intro_dat_headers \
         (snapshot_key,source_order,source_line,source_column,id_text,name,description,version_text,date,author,homepage,url,trademarks,piracy,subset,comment) \
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<BigInt, _>(checked_len(header.source_order, "No-Intro DAT header order")?)
    .bind::<BigInt, _>(header.location.line)
    .bind::<BigInt, _>(header.location.column)
    .bind::<Nullable<Text>, _>(header.id.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(header.name.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(header.description.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(header.version.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(header.date.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(header.author.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(header.homepage.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(header.url.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(header.trademarks.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(header.piracy.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(header.subset.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(header.comment.as_ref().map(DeclaredText::as_str))
    .execute(conn)?;

    for (kind, field) in [
        (HeaderField::Id, header.id.as_ref()),
        (HeaderField::Name, header.name.as_ref()),
        (HeaderField::Description, header.description.as_ref()),
        (HeaderField::Version, header.version.as_ref()),
        (HeaderField::Date, header.date.as_ref()),
        (HeaderField::Author, header.author.as_ref()),
        (HeaderField::Homepage, header.homepage.as_ref()),
        (HeaderField::Url, header.url.as_ref()),
        (HeaderField::Trademarks, header.trademarks.as_ref()),
        (HeaderField::Piracy, header.piracy.as_ref()),
        (HeaderField::Subset, header.subset.as_ref()),
        (HeaderField::Comment, header.comment.as_ref()),
    ] {
        if let Some(field) = field {
            insert_header_position(conn, snapshot_key, kind, field)?;
        }
    }
    if let Some(options) = &header.clrmamepro {
        sql_query(
            "INSERT INTO no_intro_dat_clrmamepro_options \
             (snapshot_key,source_order,source_line,source_column,forcenodump_text,header_text) VALUES (?,?,?,?,?,?)",
        )
        .bind::<Text, _>(snapshot_key.as_str())
        .bind::<BigInt, _>(checked_len(options.source_order, "No-Intro DAT clrmamepro order")?)
        .bind::<BigInt, _>(options.location.line)
        .bind::<BigInt, _>(options.location.column)
        .bind::<Nullable<Text>, _>(options.forcenodump.as_ref().map(DeclaredText::as_str))
        .bind::<Nullable<Text>, _>(options.header.as_ref().map(DeclaredText::as_str))
        .execute(conn)?;
        for (kind, field) in [
            (ClrMameProField::ForceNoDump, options.forcenodump.as_ref()),
            (ClrMameProField::Header, options.header.as_ref()),
        ] {
            if let Some(field) = field {
                insert_clrmamepro_position(conn, snapshot_key, kind, field)?;
            }
        }
    }
    if let Some(options) = &header.romcenter {
        sql_query(
            "INSERT INTO no_intro_dat_romcenter_options \
             (snapshot_key,source_order,source_line,source_column,plugin_text) VALUES (?,?,?,?,?)",
        )
        .bind::<Text, _>(snapshot_key.as_str())
        .bind::<BigInt, _>(checked_len(
            options.source_order,
            "No-Intro DAT romcenter order",
        )?)
        .bind::<BigInt, _>(options.location.line)
        .bind::<BigInt, _>(options.location.column)
        .bind::<Nullable<Text>, _>(options.plugin.as_ref().map(DeclaredText::as_str))
        .execute(conn)?;
        if let Some(field) = &options.plugin {
            insert_romcenter_position(conn, snapshot_key, RomCenterField::Plugin, field)?;
        }
    }
    ensure_root_group(conn, snapshot_key)?;
    Ok(())
}

pub(super) fn insert_game(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    game: &Game,
    digest_scope: DigestScope,
) -> crate::Result<()> {
    ensure_root_group(conn, snapshot_key)?;
    let group = sql_query(
        "SELECT set_group_id AS value FROM catalog_set_groups WHERE snapshot_key = ? AND kind = 'root'",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .get_result::<Id>(conn)?;
    let set_id = sql_query(
        "INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) \
         VALUES (?,'no_intro_dat_game',?,?,?,?) RETURNING set_id AS value",
    )
    .bind::<BigInt, _>(group.value)
    .bind::<BigInt, _>(checked_len(game.list_order, "No-Intro DAT game order")?)
    .bind::<Text, _>(&game.name.value)
    .bind::<BigInt, _>(game.location.line)
    .bind::<BigInt, _>(game.location.column)
    .get_result::<Id>(conn)?;
    sql_query(
        "INSERT INTO no_intro_dat_games(set_id,source_order,id_text,cloneof_text,cloneofid_text,description_text) VALUES (?,?,?,?,?,?)",
    )
    .bind::<BigInt, _>(set_id.value)
    .bind::<BigInt, _>(checked_len(game.source_order, "No-Intro DAT game order")?)
    .bind::<Nullable<Text>, _>(game.id.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(game.cloneof.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(game.cloneofid.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(game.description.as_ref().map(DeclaredText::as_str))
    .execute(conn)?;

    for (kind, field) in [
        (GameField::Name, Some(&game.name)),
        (GameField::Id, game.id.as_ref()),
        (GameField::CloneOf, game.cloneof.as_ref()),
        (GameField::CloneOfId, game.cloneofid.as_ref()),
        (GameField::Description, game.description.as_ref()),
    ] {
        if let Some(field) = field {
            insert_game_position(conn, set_id.value, kind, field)?;
        }
    }
    for (order, category) in game.categories.iter().enumerate() {
        sql_query("INSERT INTO no_intro_dat_categories(set_id,category_order,category,source_order,source_line,source_column) VALUES (?,?,?,?,?,?)")
            .bind::<BigInt, _>(set_id.value)
            .bind::<BigInt, _>(checked_len(order, "No-Intro DAT category order")?)
            .bind::<Text, _>(&category.value)
            .bind::<BigInt, _>(checked_len(category.source_order, "No-Intro DAT category source order")?)
            .bind::<BigInt, _>(category.location.line)
            .bind::<BigInt, _>(category.location.column)
            .execute(conn)?;
    }
    for (order, identifier) in game.identifiers.iter().enumerate() {
        sql_query("INSERT INTO no_intro_dat_identifiers(set_id,identifier_order,identifier,source_order,source_line,source_column) VALUES (?,?,?,?,?,?)")
            .bind::<BigInt, _>(set_id.value)
            .bind::<BigInt, _>(checked_len(order, "No-Intro DAT identifier order")?)
            .bind::<Text, _>(&identifier.value)
            .bind::<BigInt, _>(checked_len(identifier.source_order, "No-Intro DAT identifier source order")?)
            .bind::<BigInt, _>(identifier.location.line)
            .bind::<BigInt, _>(identifier.location.column)
            .execute(conn)?;
    }
    for (order, release) in game.releases.iter().enumerate() {
        insert_release(conn, set_id.value, order, release)?;
    }
    for (order, rom) in game.roms.iter().enumerate() {
        insert_rom(conn, set_id.value, order, rom, digest_scope)?;
    }
    Ok(())
}

pub(super) fn seal_document(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    counts: &ImportCounts,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO no_intro_dat_parse_counts \
         (snapshot_key,game_count,rom_count,category_count,identifier_count,release_count, \
          clrmamepro_option_count,romcenter_option_count, \
          header_field_count,clrmamepro_field_count,romcenter_field_count,game_field_count,rom_field_count) \
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<BigInt, _>(counts.games)
    .bind::<BigInt, _>(counts.roms)
    .bind::<BigInt, _>(counts.categories)
    .bind::<BigInt, _>(counts.identifiers)
    .bind::<BigInt, _>(counts.releases)
    .bind::<BigInt, _>(counts.clrmamepro_options)
    .bind::<BigInt, _>(counts.romcenter_options)
    .bind::<BigInt, _>(counts.header_fields)
    .bind::<BigInt, _>(counts.clrmamepro_fields)
    .bind::<BigInt, _>(counts.romcenter_fields)
    .bind::<BigInt, _>(counts.game_fields)
    .bind::<BigInt, _>(counts.rom_fields)
    .execute(conn)?;
    Ok(())
}

#[derive(QueryableByName)]
struct Id {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(QueryableByName)]
struct Format {
    #[diesel(sql_type = Text)]
    format: String,
}

#[derive(QueryableByName)]
struct Occurrence {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
}

fn require_mode(conn: &mut SqliteConnection, snapshot: &SnapshotKey) -> crate::Result<()> {
    let row = sql_query(
        "SELECT format FROM catalog_snapshots JOIN parser_interpretations USING (interpretation_key) WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<Format>(conn)?;
    if MODES.contains(&row.format.as_str()) {
        Ok(())
    } else {
        Err(crate::Error::DatabaseSchema(format!(
            "unsupported No-Intro DAT interpretation {}",
            row.format
        )))
    }
}

fn ensure_root_group(conn: &mut SqliteConnection, snapshot: &SnapshotKey) -> crate::Result<()> {
    let existing = sql_query(
        "SELECT set_group_id AS value FROM catalog_set_groups WHERE snapshot_key = ? AND kind = 'root'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<Id>(conn)
    .optional()?;
    if existing.is_none() {
        sql_query(
            "INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES (?,'root',0)",
        )
        .bind::<Text, _>(snapshot.as_str())
        .execute(conn)?;
    }
    Ok(())
}

fn insert_release(
    conn: &mut SqliteConnection,
    set_id: i64,
    order: usize,
    release: &Release,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO no_intro_dat_releases \
         (set_id,release_order,name,region,source_order,source_line,source_column, \
          name_source_order,name_source_line,name_source_column,region_source_order,region_source_line,region_source_column) \
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind::<BigInt, _>(set_id)
    .bind::<BigInt, _>(checked_len(order, "No-Intro DAT release order")?)
    .bind::<Text, _>(&release.name.value)
    .bind::<Text, _>(&release.region.value)
    .bind::<BigInt, _>(checked_len(release.source_order, "No-Intro DAT release source order")?)
    .bind::<BigInt, _>(release.location.line)
    .bind::<BigInt, _>(release.location.column)
    .bind::<BigInt, _>(checked_len(release.name.source_order, "No-Intro DAT release name source order")?)
    .bind::<BigInt, _>(release.name.location.line)
    .bind::<BigInt, _>(release.name.location.column)
    .bind::<BigInt, _>(checked_len(release.region.source_order, "No-Intro DAT release region source order")?)
    .bind::<BigInt, _>(release.region.location.line)
    .bind::<BigInt, _>(release.region.location.column)
    .execute(conn)?;
    Ok(())
}

fn insert_rom(
    conn: &mut SqliteConnection,
    set_id: i64,
    order: usize,
    rom: &Rom,
    document_scope: DigestScope,
) -> crate::Result<()> {
    let size = rom.size.as_ref().and_then(|field| parse_size(&field.value));
    let crc = digest_bytes(rom.crc.as_ref(), 4);
    let md5 = digest_bytes(rom.md5.as_ref(), 16);
    let sha1 = digest_bytes(rom.sha1.as_ref(), 20);
    let sha256 = digest_bytes(rom.sha256.as_ref(), 32);
    let scope = if rom.header.is_some() || document_scope == DigestScope::Unknown {
        DigestScope::Unknown.as_str()
    } else {
        document_scope.as_str()
    };
    let digests = ContentDigestAssertions::new(
        scope,
        crc.as_deref(),
        md5.as_deref(),
        sha1.as_deref(),
        sha256.as_deref(),
    );
    let uninterpretable_declaration = [
        rom.size.is_some() && size.is_none(),
        rom.crc.is_some() && crc.is_none(),
        rom.md5.is_some() && md5.is_none(),
        rom.sha1.is_some() && sha1.is_none(),
        rom.sha256.is_some() && sha256.is_none(),
    ]
    .into_iter()
    .any(|invalid| invalid);
    let resolution = if uninterpretable_declaration {
        ContentIdentityResolution::NoEligibleEvidence
    } else {
        resolve_content_identity(conn, size, digests)?
    };
    let content_uuid = match &resolution {
        ContentIdentityResolution::Linked(id) => Some(id.as_bytes().to_vec()),
        ContentIdentityResolution::NoEligibleEvidence
        | ContentIdentityResolution::Conflict { .. } => None,
    };
    let occurrence = sql_query(
        "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) \
         VALUES (?,?,'no_intro_dat_rom',?) RETURNING occurrence_id",
    )
    .bind::<BigInt, _>(set_id)
    .bind::<BigInt, _>(checked_len(order, "No-Intro DAT ROM order")?)
    .bind::<Nullable<Binary>, _>(content_uuid)
    .get_result::<Occurrence>(conn)?;
    let occurrence_id = OccurrenceId::from_database(occurrence.occurrence_id);
    sql_query(
        "INSERT INTO no_intro_dat_rom_claims \
         (occurrence_id,name,size_text,status_text,serial_text,header_text,date_text,mia_text, \
          evidence_scope,source_order,source_line,source_column) \
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind::<BigInt, _>(occurrence.occurrence_id)
    .bind::<Text, _>(&rom.name.value)
    .bind::<Nullable<Text>, _>(rom.size.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(rom.status.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(rom.serial.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(rom.header.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(rom.date.as_ref().map(DeclaredText::as_str))
    .bind::<Nullable<Text>, _>(rom.mia.as_ref().map(DeclaredText::as_str))
    .bind::<Text, _>(scope)
    .bind::<BigInt, _>(checked_len(
        rom.source_order,
        "No-Intro DAT ROM source order",
    )?)
    .bind::<BigInt, _>(rom.location.line)
    .bind::<BigInt, _>(rom.location.column)
    .execute(conn)?;
    record_occurrence_digest_assertions(conn, occurrence_id, digests, "source_declared")?;
    for (kind, field, interpreted) in [
        (RomDigestField::Crc, rom.crc.as_ref(), crc.as_deref()),
        (RomDigestField::Md5, rom.md5.as_ref(), md5.as_deref()),
        (RomDigestField::Sha1, rom.sha1.as_ref(), sha1.as_deref()),
        (
            RomDigestField::Sha256,
            rom.sha256.as_ref(),
            sha256.as_deref(),
        ),
    ] {
        if let Some(field) = field {
            let declaration = interpreted.map_or(
                DeclaredDigest::Invalid(&field.value),
                DeclaredDigest::Usable,
            );
            insert_digest_field(conn, occurrence.occurrence_id, kind, declaration)?;
        }
    }
    insert_rom_positions(conn, occurrence.occurrence_id, rom)?;
    record_content_identity_conflict(conn, occurrence_id, &resolution)?;
    Ok(())
}

fn insert_rom_positions(
    conn: &mut SqliteConnection,
    occurrence_id: i64,
    rom: &Rom,
) -> crate::Result<()> {
    for (kind, field) in [
        (RomField::Name, Some(&rom.name)),
        (RomField::Size, rom.size.as_ref()),
        (RomField::Crc, rom.crc.as_ref()),
        (RomField::Md5, rom.md5.as_ref()),
        (RomField::Sha1, rom.sha1.as_ref()),
        (RomField::Sha256, rom.sha256.as_ref()),
        (RomField::Status, rom.status.as_ref()),
        (RomField::Serial, rom.serial.as_ref()),
        (RomField::Header, rom.header.as_ref()),
        (RomField::Date, rom.date.as_ref()),
        (RomField::Mia, rom.mia.as_ref()),
    ] {
        if let Some(field) = field {
            insert_rom_position(conn, occurrence_id, kind, field)?;
        }
    }
    Ok(())
}

fn insert_digest_field(
    conn: &mut SqliteConnection,
    occurrence_id: i64,
    kind: RomDigestField,
    declaration: DeclaredDigest<'_>,
) -> crate::Result<()> {
    let (digest_id, invalid_text) = match declaration {
        DeclaredDigest::Usable(bytes) => {
            let id = sql_query(
                "SELECT digest_id AS value FROM digest_values WHERE algorithm=? AND digest=?",
            )
            .bind::<Text, _>(kind.algorithm())
            .bind::<Binary, _>(bytes)
            .get_result::<Id>(conn)?
            .value;
            (Some(id), None)
        }
        DeclaredDigest::Invalid(text) => (None, Some(text)),
    };
    sql_query("INSERT INTO no_intro_dat_rom_digest_fields(occurrence_id,field_kind,digest_id,invalid_text) VALUES(?,?,?,?)")
        .bind::<BigInt, _>(occurrence_id)
        .bind::<BigInt, _>(kind.position_kind() as i64)
        .bind::<Nullable<BigInt>, _>(digest_id)
        .bind::<Nullable<Text>, _>(invalid_text)
        .execute(conn)?;
    Ok(())
}

fn insert_header_position(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    field_kind: HeaderField,
    field: &DeclaredText,
) -> crate::Result<()> {
    sql_query("INSERT INTO no_intro_dat_header_field_positions(snapshot_key,field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?)")
    .bind::<Text, _>(snapshot.as_str())
    .bind::<BigInt, _>(field_kind as i64)
    .bind::<BigInt, _>(checked_len(field.source_order, "No-Intro DAT field order")?)
    .bind::<BigInt, _>(field.location.line)
    .bind::<BigInt, _>(field.location.column)
    .execute(conn)?;
    Ok(())
}

fn insert_clrmamepro_position(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    field_kind: ClrMameProField,
    field: &DeclaredText,
) -> crate::Result<()> {
    sql_query("INSERT INTO no_intro_dat_clrmamepro_field_positions(snapshot_key,field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?)")
        .bind::<Text, _>(snapshot.as_str())
        .bind::<BigInt, _>(field_kind as i64)
        .bind::<BigInt, _>(checked_len(field.source_order, "No-Intro DAT field order")?)
        .bind::<BigInt, _>(field.location.line)
        .bind::<BigInt, _>(field.location.column)
        .execute(conn)?;
    Ok(())
}

fn insert_romcenter_position(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    field_kind: RomCenterField,
    field: &DeclaredText,
) -> crate::Result<()> {
    sql_query("INSERT INTO no_intro_dat_romcenter_field_positions(snapshot_key,field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?)")
        .bind::<Text, _>(snapshot.as_str())
        .bind::<BigInt, _>(field_kind as i64)
        .bind::<BigInt, _>(checked_len(field.source_order, "No-Intro DAT field order")?)
        .bind::<BigInt, _>(field.location.line)
        .bind::<BigInt, _>(field.location.column)
        .execute(conn)?;
    Ok(())
}

fn insert_game_position(
    conn: &mut SqliteConnection,
    set_id: i64,
    field_kind: GameField,
    field: &DeclaredText,
) -> crate::Result<()> {
    sql_query("INSERT INTO no_intro_dat_game_field_positions(set_id,field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?)")
        .bind::<BigInt, _>(set_id)
        .bind::<BigInt, _>(field_kind as i64)
        .bind::<BigInt, _>(checked_len(field.source_order, "No-Intro DAT field order")?)
        .bind::<BigInt, _>(field.location.line)
        .bind::<BigInt, _>(field.location.column)
        .execute(conn)?;
    Ok(())
}

fn insert_rom_position(
    conn: &mut SqliteConnection,
    occurrence_id: i64,
    field_kind: RomField,
    field: &DeclaredText,
) -> crate::Result<()> {
    sql_query("INSERT INTO no_intro_dat_rom_field_positions(occurrence_id,field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?)")
        .bind::<BigInt, _>(occurrence_id)
        .bind::<BigInt, _>(field_kind as i64)
        .bind::<BigInt, _>(checked_len(field.source_order, "No-Intro DAT field order")?)
        .bind::<BigInt, _>(field.location.line)
        .bind::<BigInt, _>(field.location.column)
        .execute(conn)?;
    Ok(())
}

fn parse_size(value: &str) -> Option<i64> {
    let normalized = value.trim_matches([' ', '\t', '\r', '\n']);
    let parsed = normalized.parse::<i64>().ok()?;
    (parsed >= 0).then_some(parsed)
}

fn digest_bytes(field: Option<&DeclaredText>, expected_bytes: usize) -> Option<Vec<u8>> {
    let field = field?;
    if field.value.len() != expected_bytes * 2 {
        return None;
    }
    let bytes = hex::decode(&field.value).ok()?;
    (bytes.len() == expected_bytes).then_some(bytes)
}

fn checked_len(value: usize, label: &str) -> crate::Result<i64> {
    i64::try_from(value).map_err(|_| crate::Error::InvalidPath(format!("too many {label}")))
}

fn checked_add(target: &mut i64, value: i64, label: &str) -> crate::Result<()> {
    *target = target
        .checked_add(value)
        .ok_or_else(|| crate::Error::InvalidPath(format!("too many {label}")))?;
    Ok(())
}

fn present_count<const N: usize>(values: [bool; N]) -> i64 {
    values.into_iter().map(i64::from).sum()
}
