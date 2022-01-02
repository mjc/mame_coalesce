use diesel::SqliteConnection;

use crate::schema::data_files;

#[derive(Identifiable, Queryable, Associations, PartialEq, Debug)]
pub struct DataFile {
    id: i32,
    build: Option<String>,
    debug: Option<String>,
    file_name: Option<String>,
    name: String,
    description: Option<String>,
    category: Option<String>,
    version: Option<String>,
    author: Option<String>,
    email: Option<String>,
    homepage: Option<String>,
    url: Option<String>,
    sha1: Option<Vec<u8>>,
}
