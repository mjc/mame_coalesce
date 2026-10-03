use super::{
    QueryResult, SnapshotKey, SoftwareAreaAttributePositions, SoftwareAreaFields,
    SoftwareDataAreaAttribute, SoftwareDipSwitchAttribute, SoftwareDipValueAttribute,
    SoftwareDiskAreaAttribute, SoftwareEnvelope, SoftwareItemAttribute, SoftwareList,
    SoftwareListAttribute, SoftwareNamedValueAttribute, SoftwarePartAttribute, SoftwareTitle,
    SoftwareWrapperAttribute, SqliteConnection, XmlAttributePosition, drop_owner_request_table,
    load_owner_request_table,
};
use crate::storage::software_attributes::{Family, Positions};
use diesel::{
    QueryableByName, RunQueryDsl, sql_query,
    sql_types::{BigInt, Text},
};

pub(super) fn lists(conn: &mut SqliteConnection, lists: &mut [SoftwareList]) -> QueryResult<()> {
    let ids = lists
        .iter()
        .map(|list| list.id.database_value())
        .collect::<Vec<_>>();
    load_owner_request_table(conn, &ids)?;
    let mut positions = Positions::default();
    positions.load(conn,Family::List,"FROM temp.catalog_software_requested_owners AS requested CROSS JOIN __NATIVE_POSITIONS__ AS position WHERE position.namespace_id=requested.owner_id")?;
    drop_owner_request_table(conn)?;
    for list in lists {
        list.attribute_positions = positions.take(
            Family::List,
            (list.id.database_value(), 0, 0),
            SoftwareListAttribute::from_code,
            &[true, list.description.is_some()],
        )?;
    }
    positions.finish()?;
    Ok(())
}

#[derive(QueryableByName)]
struct WrapperRow {
    #[diesel(sql_type=BigInt)]
    wrapper_id: i64,
}
pub(super) fn wrapper(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    envelope: &SoftwareEnvelope,
) -> QueryResult<Vec<XmlAttributePosition<SoftwareWrapperAttribute>>> {
    match envelope {
        SoftwareEnvelope::SingleList => Ok(Vec::new()),
        SoftwareEnvelope::PluralLists { build } => {
            let row =
                sql_query("SELECT wrapper_id FROM software_wrapper_headers WHERE snapshot_key=?")
                    .bind::<Text, _>(snapshot.as_str())
                    .get_result::<WrapperRow>(conn)?;
            load_owner_request_table(conn, &[row.wrapper_id])?;
            let mut positions = Positions::default();
            positions.load(conn,Family::Wrapper,"FROM temp.catalog_software_requested_owners AS requested CROSS JOIN __NATIVE_POSITIONS__ AS position WHERE position.wrapper_id=requested.owner_id")?;
            drop_owner_request_table(conn)?;
            let result = positions.take(
                Family::Wrapper,
                (row.wrapper_id, 0, 0),
                SoftwareWrapperAttribute::from_code,
                &[build.is_some()],
            )?;
            positions.finish()?;
            Ok(result)
        }
    }
}

pub(super) fn titles(conn: &mut SqliteConnection) -> QueryResult<Positions> {
    let mut result = Positions::default();
    result.load(conn,Family::Item,"FROM temp.catalog_software_requested_owners AS requested CROSS JOIN __NATIVE_POSITIONS__ AS position WHERE position.record_id=requested.owner_id")?;
    result.load(conn,Family::Info,"FROM temp.catalog_software_requested_owners AS requested CROSS JOIN __NATIVE_POSITIONS__ AS position WHERE position.record_id=requested.owner_id")?;
    result.load(conn,Family::SharedFeature,"FROM temp.catalog_software_requested_owners AS requested CROSS JOIN __NATIVE_POSITIONS__ AS position WHERE position.record_id=requested.owner_id")?;
    result.load(conn,Family::Part,"FROM temp.catalog_software_requested_owners AS requested CROSS JOIN software_parts AS part ON part.record_id=requested.owner_id CROSS JOIN __NATIVE_POSITIONS__ AS position WHERE position.part_id=part.part_id")?;
    result.load(conn,Family::Feature,"FROM temp.catalog_software_requested_owners AS requested CROSS JOIN software_parts AS part ON part.record_id=requested.owner_id CROSS JOIN __NATIVE_POSITIONS__ AS position WHERE position.part_id=part.part_id")?;
    result.load(conn,Family::DipSwitch,"FROM temp.catalog_software_requested_owners AS requested CROSS JOIN software_parts AS part ON part.record_id=requested.owner_id CROSS JOIN __NATIVE_POSITIONS__ AS position WHERE position.part_id=part.part_id")?;
    result.load(conn,Family::DipValue,"FROM temp.catalog_software_requested_owners AS requested CROSS JOIN software_parts AS part ON part.record_id=requested.owner_id CROSS JOIN __NATIVE_POSITIONS__ AS position WHERE position.part_id=part.part_id")?;
    result.load(conn,Family::DataArea,"FROM temp.catalog_software_requested_owners AS requested CROSS JOIN software_parts AS part ON part.record_id=requested.owner_id CROSS JOIN software_areas AS area ON area.part_id=part.part_id CROSS JOIN __NATIVE_POSITIONS__ AS position WHERE position.area_id=area.area_id")?;
    result.load(conn,Family::DiskArea,"FROM temp.catalog_software_requested_owners AS requested CROSS JOIN software_parts AS part ON part.record_id=requested.owner_id CROSS JOIN software_areas AS area ON area.part_id=part.part_id CROSS JOIN __NATIVE_POSITIONS__ AS position WHERE position.area_id=area.area_id")?;
    Ok(result)
}

pub(super) fn attach_titles(
    positions: &mut Positions,
    titles: &mut [SoftwareTitle],
) -> QueryResult<()> {
    for title in titles {
        let id = title.id.as_i64();
        title.attribute_positions = positions.take(
            Family::Item,
            (id, 0, 0),
            SoftwareItemAttribute::from_code,
            &[true, title.clone_of.is_some(), title.supported_specified],
        )?;
        for (family, values) in [
            (Family::Info, &mut title.info),
            (Family::SharedFeature, &mut title.shared_features),
        ] {
            for value in values {
                value.attribute_positions = positions.take(
                    family,
                    (id, value.order, 0),
                    SoftwareNamedValueAttribute::from_code,
                    &[true, value.value.is_some()],
                )?;
            }
        }
        for part in &mut title.parts {
            let id = part.id.database_value();
            part.attribute_positions = positions.take(
                Family::Part,
                (id, 0, 0),
                SoftwarePartAttribute::from_code,
                &[true, true],
            )?;
            for value in &mut part.features {
                value.attribute_positions = positions.take(
                    Family::Feature,
                    (id, value.order, 0),
                    SoftwareNamedValueAttribute::from_code,
                    &[true, value.value.is_some()],
                )?;
            }
            for switch in &mut part.switches {
                switch.attribute_positions = positions.take(
                    Family::DipSwitch,
                    (id, switch.order, 0),
                    SoftwareDipSwitchAttribute::from_code,
                    &[true, true, true],
                )?;
                for value in &mut switch.values {
                    value.attribute_positions = positions.take(
                        Family::DipValue,
                        (id, switch.order, value.order),
                        SoftwareDipValueAttribute::from_code,
                        &[true, true, value.default_specified],
                    )?;
                }
            }
            for area in &mut part.areas {
                let id = area.id.database_value();
                area.attribute_positions = match &area.fields {
                    SoftwareAreaFields::Data {
                        width_specified,
                        endianness_specified,
                        ..
                    } => SoftwareAreaAttributePositions::Data(positions.take(
                        Family::DataArea,
                        (id, 0, 0),
                        SoftwareDataAreaAttribute::from_code,
                        &[true, true, *width_specified, *endianness_specified],
                    )?),
                    SoftwareAreaFields::Disk => {
                        SoftwareAreaAttributePositions::Disk(positions.take(
                            Family::DiskArea,
                            (id, 0, 0),
                            SoftwareDiskAreaAttribute::from_code,
                            &[true],
                        )?)
                    }
                };
            }
        }
    }
    Ok(())
}
