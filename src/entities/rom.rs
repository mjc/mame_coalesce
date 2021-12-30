use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "roms")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub size: u64,
    pub md5: Vec<u8>,
    pub sha1: Vec<u8>,
    pub crc: Vec<u8>,
    pub date: String,
    pub updated_at: String,
    pub inserted_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
