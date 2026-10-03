use crate::xml_reader::AttributePosition;
use diesel::{RunQueryDsl, SqliteConnection, sql_query, sql_types::BigInt};

#[derive(Clone, Copy)]
pub(super) enum PositionOwner {
    Wrapper(i64),
    List(i64),
    Item(i64),
    Info(i64, i64),
    SharedFeature(i64, i64),
    Part(i64),
    Feature(i64, i64),
    DataArea(i64),
    DiskArea(i64),
    Rom(i64),
    Disk(i64),
    DipSwitch(i64, i64),
    DipValue(i64, i64, i64),
}

impl PositionOwner {
    const fn query(&self) -> &'static str {
        match self {
            Self::Wrapper(..) => {
                "INSERT INTO software_wrapper_attribute_positions(wrapper_id,field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?)"
            }
            Self::List(..) => {
                "INSERT INTO software_list_attribute_positions(namespace_id,field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?)"
            }
            Self::Item(..) => {
                "INSERT INTO software_item_attribute_positions(record_id,field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?)"
            }
            Self::Info(..) => {
                "INSERT INTO software_item_info_attribute_positions(record_id,value_order,field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?,?)"
            }
            Self::SharedFeature(..) => {
                "INSERT INTO software_item_shared_feature_attribute_positions(record_id,value_order,field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?,?)"
            }
            Self::Part(..) => {
                "INSERT INTO software_part_attribute_positions(part_id,field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?)"
            }
            Self::Feature(..) => {
                "INSERT INTO software_part_feature_attribute_positions(part_id,value_order,field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?,?)"
            }
            Self::DataArea(..) => {
                "INSERT INTO software_data_area_attribute_positions(area_id,field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?)"
            }
            Self::DiskArea(..) => {
                "INSERT INTO software_disk_area_attribute_positions(area_id,field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?)"
            }
            Self::Rom(..) => {
                "INSERT INTO software_rom_attribute_positions(occurrence_id,field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?)"
            }
            Self::Disk(..) => {
                "INSERT INTO software_disk_attribute_positions(occurrence_id,field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?)"
            }
            Self::DipSwitch(..) => {
                "INSERT INTO software_part_dipswitch_attribute_positions(part_id,dipswitch_order,field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?,?)"
            }
            Self::DipValue(..) => {
                "INSERT INTO software_part_dip_value_attribute_positions(part_id,dipswitch_order,value_order,field_kind,source_order,source_line,source_column) VALUES (?,?,?,?,?,?,?)"
            }
        }
    }
    fn keys(&self) -> Vec<i64> {
        match *self {
            Self::Wrapper(k0)
            | Self::List(k0)
            | Self::Item(k0)
            | Self::Part(k0)
            | Self::DataArea(k0)
            | Self::DiskArea(k0)
            | Self::Rom(k0)
            | Self::Disk(k0) => vec![k0],
            Self::Info(k0, k1)
            | Self::SharedFeature(k0, k1)
            | Self::Feature(k0, k1)
            | Self::DipSwitch(k0, k1) => vec![k0, k1],
            Self::DipValue(k0, k1, k2) => vec![k0, k1, k2],
        }
    }
}

pub(super) fn insert<Field: Copy>(
    conn: &mut SqliteConnection,
    owner: PositionOwner,
    positions: &[AttributePosition<Field>],
    code: fn(Field) -> i64,
) -> crate::Result<()> {
    let keys = owner.keys();
    for position in positions {
        let mut query = sql_query(owner.query()).into_boxed::<diesel::sqlite::Sqlite>();
        for key in &keys {
            query = query.bind::<BigInt, _>(*key);
        }
        query
            .bind::<BigInt, _>(code(position.field))
            .bind::<BigInt, _>(i64::try_from(position.source_order).map_err(|_| {
                crate::Error::XmlValidation(
                    "software attribute ordinal exceeds storage range".into(),
                )
            })?)
            .bind::<BigInt, _>(position.location.line)
            .bind::<BigInt, _>(position.location.column)
            .execute(conn)?;
    }
    Ok(())
}
