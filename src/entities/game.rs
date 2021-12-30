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
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
