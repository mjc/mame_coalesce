use diesel::{Identifiable, Insertable, Queryable};

use crate::{logiqx, storage::schema::data_files};

#[derive(Identifiable, Queryable, PartialEq, Eq, Debug)]
#[diesel(table_name = data_files)]
pub struct DataFile {
    pub id: i32,
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

impl DataFile {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Insertable)]
#[diesel(table_name = data_files)]
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
    sha1: Option<&'a [u8]>,
}

impl New<'_> {
    pub fn from_logiqx(l_data_file: &logiqx::DataFile) -> crate::Result<New<'_>> {
        let header = l_data_file.header()?;
        Ok(New {
            build: l_data_file.build().map(str::to_owned),
            debug: l_data_file.debug().map(str::to_owned),
            file_name: l_data_file.file_name().map(str::to_owned),
            name: header.name().to_owned(),
            description: header.description().cloned(),
            version: header.version().cloned(),
            author: header.author().cloned(),
            homepage: header.homepage().cloned(),
            url: header.url().cloned(),
            sha1: l_data_file.sha1(),
        })
    }

    #[must_use]
    pub const fn sha1(&self) -> Option<&[u8]> {
        self.sha1
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}
