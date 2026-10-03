use crate::{
    domain::{CatalogSetId, SnapshotKey},
    logiqx::RecordLocation,
    no_intro_pc_xml::{AttributePosition, GameAttribute, Header, RomAttribute},
    xml_reader::DeclaredText,
};
use diesel::{
    RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Bool, Nullable, Text},
};

pub(super) struct DocumentFacts {
    pub(super) location: RecordLocation,
    pub(super) header: Option<Header>,
}

pub(super) fn insert_game_positions(
    conn: &mut SqliteConnection,
    owner: CatalogSetId,
    positions: &[AttributePosition<GameAttribute>],
) -> crate::Result<()> {
    insert_positions(
        conn,
        "INSERT INTO no_intro_pc_game_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,?,?,?,?)",
        owner.as_i64(),
        positions.iter().map(|position| {
            (
                position.field.code(),
                position.source_order,
                position.location,
            )
        }),
    )
}

pub(super) fn insert_rom_positions(
    conn: &mut SqliteConnection,
    owner: crate::storage::catalog_identity::OccurrenceId,
    positions: &[AttributePosition<RomAttribute>],
) -> crate::Result<()> {
    insert_positions(
        conn,
        "INSERT INTO no_intro_pc_rom_attribute_positions(occurrence_id,field_kind,source_order,source_line,source_column) VALUES(?,?,?,?,?)",
        owner.database_value(),
        positions.iter().map(|position| {
            (
                position.field.code(),
                position.source_order,
                position.location,
            )
        }),
    )
}

fn insert_positions(
    conn: &mut SqliteConnection,
    statement: &'static str,
    owner: i64,
    positions: impl Iterator<Item = (i64, usize, RecordLocation)>,
) -> crate::Result<()> {
    for (field, order, location) in positions {
        sql_query(statement)
            .bind::<BigInt, _>(owner)
            .bind::<BigInt, _>(field)
            .bind::<BigInt, _>(super::checked_order(order, "P/C attributes")?)
            .bind::<BigInt, _>(location.line)
            .bind::<BigInt, _>(location.column)
            .execute(conn)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum RepeatedHeaderField {
    Name,
    Description,
}

impl RepeatedHeaderField {
    const fn owner(self) -> (&'static str, &'static str) {
        match self {
            Self::Name => ("no_intro_pc_header_names", "name_text"),
            Self::Description => ("no_intro_pc_header_descriptions", "description_text"),
        }
    }
}

pub(super) fn insert_document(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    document: &DocumentFacts,
) -> crate::Result<()> {
    sql_query("INSERT INTO no_intro_pc_documents(snapshot_key,header_present,source_line,source_column) VALUES(?,?,?,?)")
        .bind::<Text,_>(snapshot.as_str()).bind::<Bool,_>(document.header.is_some())
        .bind::<BigInt,_>(document.location.line).bind::<BigInt,_>(document.location.column).execute(conn)?;
    let Some(header) = &document.header else {
        return Ok(());
    };
    sql_query("INSERT INTO no_intro_pc_headers(snapshot_key,source_order,source_line,source_column,version_text,version_order,version_line,version_column) VALUES(?,?,?,?,?,?,?,?)")
        .bind::<Text,_>(snapshot.as_str()).bind::<BigInt,_>(super::checked_order(header.source_order,"P/C document fields")?)
        .bind::<BigInt,_>(header.location.line).bind::<BigInt,_>(header.location.column)
        .bind::<Nullable<Text>,_>(header.version.as_ref().map(DeclaredText::as_str))
        .bind::<Nullable<BigInt>,_>(header.version.as_ref().map(|field| super::checked_order(field.source_order,"P/C header fields")).transpose()?)
        .bind::<Nullable<BigInt>,_>(header.version.as_ref().map(|field| field.location.line))
        .bind::<Nullable<BigInt>,_>(header.version.as_ref().map(|field| field.location.column)).execute(conn)?;
    for (kind, fields) in [
        (RepeatedHeaderField::Name, &header.names),
        (RepeatedHeaderField::Description, &header.descriptions),
    ] {
        let (table, column) = kind.owner();
        for field in fields {
            sql_query(format!("INSERT INTO {table}(snapshot_key,source_order,{column},source_line,source_column) VALUES(?,?,?,?,?)"))
                .bind::<Text,_>(snapshot.as_str()).bind::<BigInt,_>(super::checked_order(field.source_order,"P/C header fields")?)
                .bind::<Text,_>(field.as_str()).bind::<BigInt,_>(field.location.line).bind::<BigInt,_>(field.location.column).execute(conn)?;
        }
    }
    Ok(())
}
