use super::Game;
use crate::schema::roms;

#[derive(Queryable, Insertable, AsChangeset, Associations, PartialEq, Debug)]
#[diesel(table_name = roms)]
#[belongs_to(Game)]
pub struct Rom {
    pub id: Option<i32>,
    pub name: String,
    pub md5: Vec<u8>,
    pub sha1: Vec<u8>,
    pub crc: Vec<u8>,
    pub date: String,                // utc date
    pub updated_at: Option<String>,  // utc datetime
    pub inserted_at: Option<String>, // utc datetime
    pub game_id: i32,
}
