use diesel::{
    RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Bool, Nullable, Text},
};

use crate::{
    clrmamepro::{AssetFacts, FieldValue, Header, HeaderDirectives, SetFacts},
    domain::SnapshotKey,
};

#[derive(Clone, Copy)]
#[repr(i64)]
enum HeaderField {
    Name = 0,
    Description = 1,
    Version = 2,
    Date = 3,
    Author = 4,
    Email = 5,
    Homepage = 6,
    Url = 7,
    Comment = 8,
    Category = 9,
    HeaderDefinition = 10,
    ForceMerging = 11,
    ForceZipping = 12,
    ForcePacking = 13,
    ForceNoDump = 14,
}

#[derive(Clone, Copy)]
#[repr(i64)]
enum SetField {
    Name = 0,
    CloneOf = 1,
    Description = 2,
    Year = 3,
    Manufacturer = 4,
    RebuildTo = 5,
    SampleOf = 6,
    Region = 7,
    ReleaseYear = 8,
    ReleaseMonth = 9,
    ReleaseDay = 10,
    Serial = 11,
}

#[derive(Clone, Copy)]
struct SourcePosition<'a> {
    source_field: &'a str,
    source_order: usize,
    is_quoted: bool,
    location: crate::logiqx::RecordLocation,
}

macro_rules! bind_position {
    ($query:expr, $position:expr) => {{
        let position = $position;
        $query
            .bind::<Text, _>(position.source_field)
            .bind::<BigInt, _>(checked_order(position.source_order)?)
            .bind::<Bool, _>(position.is_quoted)
            .bind::<BigInt, _>(position.location.line)
            .bind::<BigInt, _>(position.location.column)
    }};
}

fn source_position(field: &FieldValue) -> SourcePosition<'_> {
    SourcePosition {
        source_field: &field.source_name,
        source_order: field.order,
        is_quoted: field.quoted,
        location: field.location,
    }
}

fn checked_order(order: usize) -> crate::Result<i64> {
    i64::try_from(order).map_err(|_| crate::Error::InvalidPath("too many ClrMamePro fields".into()))
}

const fn field_text(field: &FieldValue) -> &str {
    field.value.as_str()
}

pub(super) fn insert_header_facts(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    header: &Header,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO cmp_header_facts (
           snapshot_key, source_block, source_order, source_line, source_column,
           name, description, version, date, author, email, homepage, url, comment, category
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<Text, _>(&header.source_name)
    .bind::<BigInt, _>(checked_order(header.source_order)?)
    .bind::<BigInt, _>(header.location.line)
    .bind::<BigInt, _>(header.location.column)
    .bind::<Nullable<Text>, _>(header.name.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(header.description.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(header.version.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(header.date.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(header.author.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(header.email.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(header.homepage.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(header.url.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(header.comment.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(header.category.as_ref().map(field_text))
    .execute(conn)?;

    insert_directives(conn, snapshot_key, &header.directives)?;
    insert_header_positions(conn, snapshot_key, header)
}

fn insert_header_positions(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    header: &Header,
) -> crate::Result<()> {
    for (kind, field) in [
        (HeaderField::Name, &header.name),
        (HeaderField::Description, &header.description),
        (HeaderField::Version, &header.version),
        (HeaderField::Date, &header.date),
        (HeaderField::Author, &header.author),
        (HeaderField::Email, &header.email),
        (HeaderField::Homepage, &header.homepage),
        (HeaderField::Url, &header.url),
        (HeaderField::Comment, &header.comment),
        (HeaderField::Category, &header.category),
        (HeaderField::HeaderDefinition, &header.directives.header),
        (HeaderField::ForceMerging, &header.directives.forcemerging),
        (HeaderField::ForceZipping, &header.directives.forcezipping),
        (HeaderField::ForcePacking, &header.directives.forcepacking),
        (HeaderField::ForceNoDump, &header.directives.forcenodump),
    ] {
        if let Some(field) = field {
            insert_header_position(conn, snapshot_key, kind, source_position(field))?;
        }
    }
    Ok(())
}

fn insert_header_position(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    kind: HeaderField,
    position: SourcePosition<'_>,
) -> crate::Result<()> {
    bind_position!(
        sql_query(
            "INSERT INTO cmp_header_field_positions (
               snapshot_key, field_kind, source_field, source_order, is_quoted, source_line, source_column
             ) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(snapshot_key.as_str())
        .bind::<BigInt, _>(kind as i64),
        position
    )
    .execute(conn)?;
    Ok(())
}

pub(super) fn insert_set_facts(
    conn: &mut SqliteConnection,
    record_id: i64,
    facts: &SetFacts,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO cmp_set_facts (
           record_id, source_block, document_order, description, year, manufacturer,
           rebuildto, region, release_year_text, release_month_text, release_day_text, serial
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(record_id)
    .bind::<Text, _>(&facts.source_block)
    .bind::<BigInt, _>(checked_order(facts.document_order)?)
    .bind::<Nullable<Text>, _>(facts.description.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(facts.year.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(facts.manufacturer.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(facts.rebuildto.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(facts.region.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(facts.release_year_text.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(facts.release_month_text.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(facts.release_day_text.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(facts.serial.as_ref().map(field_text))
    .execute(conn)?;

    if let Some(parent) = &facts.sampleof {
        sql_query("INSERT INTO cmp_sample_parent_links (record_id, target_name) VALUES (?, ?)")
            .bind::<BigInt, _>(record_id)
            .bind::<Text, _>(&parent.value)
            .execute(conn)?;
    }

    for (kind, field) in [
        (SetField::Name, &facts.name),
        (SetField::CloneOf, &facts.cloneof),
        (SetField::Description, &facts.description),
        (SetField::Year, &facts.year),
        (SetField::Manufacturer, &facts.manufacturer),
        (SetField::RebuildTo, &facts.rebuildto),
        (SetField::SampleOf, &facts.sampleof),
        (SetField::Region, &facts.region),
        (SetField::ReleaseYear, &facts.release_year_text),
        (SetField::ReleaseMonth, &facts.release_month_text),
        (SetField::ReleaseDay, &facts.release_day_text),
        (SetField::Serial, &facts.serial),
    ] {
        if let Some(field) = field {
            insert_set_position(conn, record_id, kind, source_position(field))?;
        }
    }

    Ok(())
}

pub(super) fn insert_sample(
    conn: &mut SqliteConnection,
    occurrence: crate::storage::catalog_identity::OccurrenceId,
    sample: &FieldValue,
) -> crate::Result<()> {
    bind_position!(
        sql_query(
            "INSERT INTO cmp_samples (
               occurrence_id, sample_name, source_field, source_order,
               is_quoted, source_line, source_column
             ) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<BigInt, _>(occurrence.database_value())
        .bind::<Text, _>(&sample.value),
        source_position(sample)
    )
    .execute(conn)?;
    Ok(())
}

fn insert_set_position(
    conn: &mut SqliteConnection,
    record_id: i64,
    kind: SetField,
    position: SourcePosition<'_>,
) -> crate::Result<()> {
    bind_position!(
        sql_query(
            "INSERT INTO cmp_set_field_positions (
               record_id, field_kind, source_field, source_order, is_quoted, source_line, source_column
             ) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<BigInt, _>(record_id)
        .bind::<BigInt, _>(kind as i64),
        position
    )
    .execute(conn)?;
    Ok(())
}

#[derive(Clone, Copy)]
#[repr(i64)]
enum RomField {
    Name,
    Size,
    Crc,
    Crc32,
    Md5,
    Sha1,
    Merge,
    Date,
    Serial,
    Status,
    NoDump,
    BadDump,
}

pub(super) fn insert_rom_claim(
    conn: &mut SqliteConnection,
    occurrence_id: i64,
    asset: &super::SnapshotAsset,
) -> crate::Result<()> {
    let facts = asset
        .cmp_rom_facts()
        .ok_or_else(|| crate::Error::InvalidPath("CMP ROM has no native declarations".into()))?;
    sql_query(
        "INSERT INTO cmp_rom_claims (
         occurrence_id,name,size_text,crc_text,crc32_text,md5_text,sha1_text,
         evidence_scope,evidence_provenance,merge_name,date,serial,status_text,
         nodump_present,baddump_present,source_line,source_column)
         VALUES (?,?,?,?,?,?,?,?,'source_declared',?,?,?,?,?,?,?,?)",
    )
    .bind::<BigInt, _>(occurrence_id)
    .bind::<Text, _>(&asset.name)
    .bind::<Nullable<Text>, _>(facts.size.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(facts.crc.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(facts.crc32.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(facts.md5.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(facts.sha1.as_ref().map(field_text))
    .bind::<Text, _>(asset.evidence_scope)
    .bind::<Nullable<Text>, _>(facts.merge.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(facts.date.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(facts.serial.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(facts.status_field.as_ref().map(field_text))
    .bind::<Bool, _>(facts.nodump.is_some())
    .bind::<Bool, _>(facts.baddump.is_some())
    .bind::<BigInt, _>(asset.location.line)
    .bind::<BigInt, _>(asset.location.column)
    .execute(conn)?;
    Ok(())
}

pub(super) fn insert_rom_positions(
    conn: &mut SqliteConnection,
    occurrence_id: i64,
    facts: &AssetFacts,
) -> crate::Result<()> {
    sql_query("INSERT INTO cmp_set_rom_positions (occurrence_id, source_order) VALUES (?, ?)")
        .bind::<BigInt, _>(occurrence_id)
        .bind::<BigInt, _>(checked_order(facts.set_order)?)
        .execute(conn)?;

    for (kind, field) in [
        (RomField::Name, &facts.name),
        (RomField::Size, &facts.size),
        (RomField::Crc, &facts.crc),
        (RomField::Crc32, &facts.crc32),
        (RomField::Md5, &facts.md5),
        (RomField::Sha1, &facts.sha1),
        (RomField::Merge, &facts.merge),
        (RomField::Date, &facts.date),
        (RomField::Serial, &facts.serial),
        (RomField::Status, &facts.status_field),
    ] {
        if let Some(field) = field {
            insert_rom_position(conn, occurrence_id, kind, source_position(field))?;
        }
    }
    for (kind, flag) in [
        (RomField::NoDump, &facts.nodump),
        (RomField::BadDump, &facts.baddump),
    ] {
        if let Some(flag) = flag {
            insert_rom_position(
                conn,
                occurrence_id,
                kind,
                SourcePosition {
                    source_field: &flag.source_name,
                    source_order: flag.order,
                    is_quoted: false,
                    location: flag.location,
                },
            )?;
        }
    }
    Ok(())
}

fn insert_rom_position(
    conn: &mut SqliteConnection,
    occurrence_id: i64,
    kind: RomField,
    position: SourcePosition<'_>,
) -> crate::Result<()> {
    bind_position!(
        sql_query(
            "INSERT INTO cmp_rom_field_positions (
               occurrence_id, field_kind, source_field, source_order, is_quoted, source_line, source_column
             ) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<BigInt, _>(occurrence_id)
        .bind::<BigInt, _>(kind as i64),
        position
    )
    .execute(conn)?;
    Ok(())
}

pub(super) fn insert_document_facts(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    header_present: bool,
    comments: &[crate::clrmamepro::Comment],
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO cmp_documents (snapshot_key, header_present, comment_count) VALUES (?, ?, ?)",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<Bool, _>(header_present)
    .bind::<BigInt, _>(checked_order(comments.len())?)
    .execute(conn)?;

    insert_comments(conn, snapshot_key, comments)
}

fn insert_comments(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    comments: &[crate::clrmamepro::Comment],
) -> crate::Result<()> {
    for (comment_order, comment) in comments.iter().enumerate() {
        sql_query(
            "INSERT INTO cmp_comments (snapshot_key, comment_order, text, source_line, source_column)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(snapshot_key.as_str())
        .bind::<BigInt, _>(checked_order(comment_order)?)
        .bind::<Text, _>(&comment.text)
        .bind::<BigInt, _>(comment.location.line)
        .bind::<BigInt, _>(comment.location.column)
        .execute(conn)?;
    }
    Ok(())
}

fn insert_directives(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    directives: &HeaderDirectives,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO cmp_header_directives (
           snapshot_key, header_definition, forcemerging, forcezipping, forcepacking, forcenodump
         ) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<Nullable<Text>, _>(directives.header.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(directives.forcemerging.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(directives.forcezipping.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(directives.forcepacking.as_ref().map(field_text))
    .bind::<Nullable<Text>, _>(directives.forcenodump.as_ref().map(field_text))
    .execute(conn)?;
    Ok(())
}
