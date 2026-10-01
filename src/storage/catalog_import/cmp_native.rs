use diesel::{
    RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Bool, Nullable, Text},
};

use crate::{
    clrmamepro::{AssetFacts, FieldValue, Header, HeaderDirectives, SetFacts},
    domain::SnapshotKey,
};

macro_rules! bind_field {
    ($query:expr, $field:expr) => {{
        let field = $field.as_ref();
        $query
            .bind::<Nullable<Text>, _>(field.map(|field| field.value.as_str()))
            .bind::<Nullable<BigInt>, _>(field.map(|field| field_order(field)).transpose()?)
            .bind::<Nullable<Text>, _>(field.map(|field| field.source_name.as_str()))
            .bind::<Nullable<Bool>, _>(field.map(|field| field.quoted))
            .bind::<Nullable<BigInt>, _>(field.map(|field| field.location.line))
            .bind::<Nullable<BigInt>, _>(field.map(|field| field.location.column))
    }};
}

fn field_order(field: &FieldValue) -> crate::Result<i64> {
    i64::try_from(field.order)
        .map_err(|_| crate::Error::InvalidPath("too many ClrMamePro fields".into()))
}

pub(super) fn insert_header_facts(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    header: &Header,
) -> crate::Result<()> {
    let query = sql_query(
        "INSERT INTO cmp_header_facts (
           snapshot_key, source_line, source_column,
           name, name_order, name_source_field, name_quoted, name_source_line, name_source_column,
           description, description_order, description_source_field, description_quoted, description_source_line, description_source_column,
           version, version_order, version_source_field, version_quoted, version_source_line, version_source_column,
           date, date_order, date_source_field, date_quoted, date_source_line, date_source_column,
           author, author_order, author_source_field, author_quoted, author_source_line, author_source_column,
           email, email_order, email_source_field, email_quoted, email_source_line, email_source_column,
           homepage, homepage_order, homepage_source_field, homepage_quoted, homepage_source_line, homepage_source_column,
           url, url_order, url_source_field, url_quoted, url_source_line, url_source_column,
           comment, comment_order, comment_source_field, comment_quoted, comment_source_line, comment_source_column,
           category, category_order, category_source_field, category_quoted, category_source_line, category_source_column
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<BigInt, _>(header.location.line)
    .bind::<BigInt, _>(header.location.column);
    let query = bind_field!(query, header.name);
    let query = bind_field!(query, header.description);
    let query = bind_field!(query, header.version);
    let query = bind_field!(query, header.date);
    let query = bind_field!(query, header.author);
    let query = bind_field!(query, header.email);
    let query = bind_field!(query, header.homepage);
    let query = bind_field!(query, header.url);
    let query = bind_field!(query, header.comment);
    let query = bind_field!(query, header.category);
    query.execute(conn)?;

    insert_directives(conn, snapshot_key, &header.directives)
}

pub(super) fn insert_set_facts(
    conn: &mut SqliteConnection,
    record_id: i64,
    facts: &SetFacts,
) -> crate::Result<()> {
    if facts.description.is_some()
        || facts.year.is_some()
        || facts.manufacturer.is_some()
        || facts.rebuildto.is_some()
    {
        let query = sql_query(
            "INSERT INTO cmp_set_facts (
               record_id,
               description, description_order, description_source_field, description_quoted,
               description_source_line, description_source_column,
               year, year_order, year_source_field, year_quoted, year_source_line, year_source_column,
               manufacturer, manufacturer_order, manufacturer_source_field, manufacturer_quoted,
               manufacturer_source_line, manufacturer_source_column,
               rebuildto, rebuildto_order, rebuildto_source_field, rebuildto_quoted,
               rebuildto_source_line, rebuildto_source_column
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<BigInt, _>(record_id);
        let query = bind_field!(query, facts.description);
        let query = bind_field!(query, facts.year);
        let query = bind_field!(query, facts.manufacturer);
        let query = bind_field!(query, facts.rebuildto);
        query.execute(conn)?;
    }

    if let Some(parent) = &facts.sampleof {
        sql_query(
            "INSERT INTO cmp_sample_parent_links (
               record_id, target_name, source_field, source_order, is_quoted, source_line, source_column
             ) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<BigInt, _>(record_id)
        .bind::<Text, _>(&parent.value)
        .bind::<Text, _>(&parent.source_name)
        .bind::<BigInt, _>(field_order(parent)?)
        .bind::<Bool, _>(parent.quoted)
        .bind::<BigInt, _>(parent.location.line)
        .bind::<BigInt, _>(parent.location.column)
        .execute(conn)?;
    }

    for (sample_order, sample) in facts.samples.iter().enumerate() {
        sql_query(
            "INSERT INTO cmp_samples (
               record_id, sample_order, sample_name, source_field, source_order,
               is_quoted, source_line, source_column
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<BigInt, _>(record_id)
        .bind::<BigInt, _>(checked_order(sample_order)?)
        .bind::<Text, _>(&sample.value)
        .bind::<Text, _>(&sample.source_name)
        .bind::<BigInt, _>(field_order(sample)?)
        .bind::<Bool, _>(sample.quoted)
        .bind::<BigInt, _>(sample.location.line)
        .bind::<BigInt, _>(sample.location.column)
        .execute(conn)?;
    }
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
        .cmp_facts
        .as_ref()
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

const fn field_text(field: &FieldValue) -> &str {
    field.value.as_str()
}

pub(super) fn insert_rom_positions(
    conn: &mut SqliteConnection,
    occurrence_id: i64,
    facts: &AssetFacts,
) -> crate::Result<()> {
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
            insert_rom_position(
                conn,
                occurrence_id,
                RomPosition {
                    kind,
                    source_field: &field.source_name,
                    order: field.order,
                    quoted: field.quoted,
                    location: field.location,
                },
            )?;
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
                RomPosition {
                    kind,
                    source_field: &flag.source_name,
                    order: flag.order,
                    quoted: false,
                    location: flag.location,
                },
            )?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct RomPosition<'a> {
    kind: RomField,
    source_field: &'a str,
    order: usize,
    quoted: bool,
    location: crate::logiqx::RecordLocation,
}

fn insert_rom_position(
    conn: &mut SqliteConnection,
    occurrence_id: i64,
    position: RomPosition<'_>,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO cmp_rom_field_positions(
         occurrence_id,field_kind,source_field,source_order,is_quoted,source_line,source_column)
         VALUES (?,?,?,?,?,?,?)",
    )
    .bind::<BigInt, _>(occurrence_id)
    .bind::<BigInt, _>(position.kind as i64)
    .bind::<Text, _>(position.source_field)
    .bind::<BigInt, _>(checked_order(position.order)?)
    .bind::<Bool, _>(position.quoted)
    .bind::<BigInt, _>(position.location.line)
    .bind::<BigInt, _>(position.location.column)
    .execute(conn)?;
    Ok(())
}

fn checked_order(order: usize) -> crate::Result<i64> {
    i64::try_from(order).map_err(|_| crate::Error::InvalidPath("too many ClrMamePro fields".into()))
}

fn insert_directives(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    directives: &HeaderDirectives,
) -> crate::Result<()> {
    let query = sql_query(
        "INSERT INTO cmp_header_directives (
           snapshot_key,
           header_definition, header_definition_order, header_definition_source_field, header_definition_quoted,
           header_definition_source_line, header_definition_source_column,
           forcemerging, forcemerging_order, forcemerging_source_field, forcemerging_quoted,
           forcemerging_source_line, forcemerging_source_column,
           forcezipping, forcezipping_order, forcezipping_source_field, forcezipping_quoted,
           forcezipping_source_line, forcezipping_source_column,
           forcepacking, forcepacking_order, forcepacking_source_field, forcepacking_quoted,
           forcepacking_source_line, forcepacking_source_column,
           forcenodump, forcenodump_order, forcenodump_source_field, forcenodump_quoted,
           forcenodump_source_line, forcenodump_source_column
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(snapshot_key.as_str());
    let query = bind_field!(query, directives.header);
    let query = bind_field!(query, directives.forcemerging);
    let query = bind_field!(query, directives.forcezipping);
    let query = bind_field!(query, directives.forcepacking);
    let query = bind_field!(query, directives.forcenodump);
    query.execute(conn)?;
    Ok(())
}
