use diesel::Insertable;

use crate::{logiqx, schema::data_files};

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

#[derive(Insertable, AsChangeset)]
#[table_name = "data_files"]
pub struct NewDataFile<'a> {
    build: Option<String>,
    debug: Option<String>,
    file_name: Option<String>,
    name: String,
    description: Option<String>,
    version: Option<String>,
    author: Option<String>,
    homepage: Option<String>,
    url: Option<String>,
    sha1: Option<&'a Vec<u8>>,
}

impl NewDataFile<'_> {
    pub fn from_logiqx<'a>(l_data_file: &'a logiqx::DataFile, name: &'a str) -> NewDataFile<'a> {
        NewDataFile {
            build: Some(l_data_file.build().to_string()),
            debug: Some(l_data_file.debug().to_string()),
            file_name: Some(name.to_string()),
            name: l_data_file.header().name().to_string(),
            description: Some(l_data_file.header().description().to_string()),
            version: Some(l_data_file.header().version().to_string()),
            author: Some(l_data_file.header().author().to_string()),
            homepage: Some(l_data_file.header().homepage().to_string()),
            url: Some(l_data_file.header().url().to_string()),
            sha1: l_data_file.sha1(),
        }
    }
}
