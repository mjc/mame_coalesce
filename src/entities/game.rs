use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "games")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub is_bios: bool,
    pub clone_of: i32,
    pub rom_of: i32,
    pub sample_of: i32,
    pub board: String,
    pub rebuildto: String,
    pub year: i64,
    pub manufacturer: String,
    pub data_file_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::rom::Entity")]
    Rom,
    #[sea_orm(
        belongs_to = "super::data_file::Entity",
        from = "Column::DataFileId",
        to = "super::data_file::Column::Id"
    )]
    DataFile,
}

impl Related<super::rom::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Rom.def()
    }
}

impl Related<super::data_file::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::DataFile.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
