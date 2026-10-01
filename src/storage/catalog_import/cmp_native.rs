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

pub(super) fn insert_rom_facts(
    conn: &mut SqliteConnection,
    occurrence_id: i64,
    facts: &AssetFacts,
) -> crate::Result<()> {
    if facts.date.is_none()
        && facts.serial.is_none()
        && facts.nodump_order.is_none()
        && facts.baddump_order.is_none()
        && facts.status_field.is_none()
    {
        return Ok(());
    }

    sql_query(
        "INSERT INTO cmp_rom_facts (
           occurrence_id, date, date_source_field, date_source_order, date_quoted,
           date_source_line, date_source_column,
           serial, serial_source_field, serial_source_order, serial_quoted,
           serial_source_line, serial_source_column,
           nodump_order, baddump_order, status_explicit_order, status_source_field,
           status_quoted, status_source_line, status_source_column
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(occurrence_id)
    .bind::<Nullable<Text>, _>(facts.date.as_ref().map(|field| field.value.as_str()))
    .bind::<Nullable<Text>, _>(facts.date.as_ref().map(|field| field.source_name.as_str()))
    .bind::<Nullable<BigInt>, _>(facts.date.as_ref().map(field_order).transpose()?)
    .bind::<Nullable<Bool>, _>(facts.date.as_ref().map(|field| field.quoted))
    .bind::<Nullable<BigInt>, _>(facts.date.as_ref().map(|field| field.location.line))
    .bind::<Nullable<BigInt>, _>(facts.date.as_ref().map(|field| field.location.column))
    .bind::<Nullable<Text>, _>(facts.serial.as_ref().map(|field| field.value.as_str()))
    .bind::<Nullable<Text>, _>(
        facts
            .serial
            .as_ref()
            .map(|field| field.source_name.as_str()),
    )
    .bind::<Nullable<BigInt>, _>(facts.serial.as_ref().map(field_order).transpose()?)
    .bind::<Nullable<Bool>, _>(facts.serial.as_ref().map(|field| field.quoted))
    .bind::<Nullable<BigInt>, _>(facts.serial.as_ref().map(|field| field.location.line))
    .bind::<Nullable<BigInt>, _>(facts.serial.as_ref().map(|field| field.location.column))
    .bind::<Nullable<BigInt>, _>(facts.nodump_order.map(checked_order).transpose()?)
    .bind::<Nullable<BigInt>, _>(facts.baddump_order.map(checked_order).transpose()?)
    .bind::<Nullable<BigInt>, _>(facts.status_field.as_ref().map(field_order).transpose()?)
    .bind::<Nullable<Text>, _>(
        facts
            .status_field
            .as_ref()
            .map(|field| field.source_name.as_str()),
    )
    .bind::<Nullable<Bool>, _>(facts.status_field.as_ref().map(|field| field.quoted))
    .bind::<Nullable<BigInt>, _>(facts.status_field.as_ref().map(|field| field.location.line))
    .bind::<Nullable<BigInt>, _>(
        facts
            .status_field
            .as_ref()
            .map(|field| field.location.column),
    )
    .execute(conn)?;
    Ok(())
}

fn checked_order(order: usize) -> crate::Result<i64> {
    i64::try_from(order).map_err(|_| crate::Error::InvalidPath("too many ClrMamePro fields".into()))
}

macro_rules! bind_short_field {
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
    let query = bind_short_field!(query, directives.header);
    let query = bind_short_field!(query, directives.forcemerging);
    let query = bind_short_field!(query, directives.forcezipping);
    let query = bind_short_field!(query, directives.forcepacking);
    let query = bind_short_field!(query, directives.forcenodump);
    query.execute(conn)?;
    Ok(())
}
