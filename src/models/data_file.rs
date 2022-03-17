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
pub struct New<'a> {
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

impl New<'_> {
    pub fn from_logiqx(l_data_file: &logiqx::DataFile) -> New<'_> {
        New {
            build: l_data_file.build().cloned(),
            debug: l_data_file.debug().cloned(),
            file_name: l_data_file.file_name().cloned(),
            name: l_data_file.header().name().to_owned(),
            description: l_data_file.header().description().cloned(),
            version: l_data_file.header().version().cloned(),
            author: l_data_file.header().author().cloned(),
            homepage: l_data_file.header().homepage().cloned(),
            url: l_data_file.header().url().cloned(),
            sha1: l_data_file.sha1(),
        }
    }

    /// Get a reference to the new data file's sha1.
    pub fn sha1(&self) -> Option<&Vec<u8>> {
        self.sha1
    }

    /// Get a reference to the new data file's name.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}
