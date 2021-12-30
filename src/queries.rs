use std::convert::TryInto;

use diesel::SqliteConnection;

use crate::logiqx;
use crate::models::*;

use crate::schema;
use diesel::prelude::*;

// this should definitely not be one giant file

pub fn traverse_and_insert_data_file(
    conn: SqliteConnection,
    data_file: logiqx::DataFile,
    file_name: &str,
) {
    let data_file_id = insert_data_file(&conn, &data_file, &file_name);
}

fn insert_data_file(
    conn: &SqliteConnection,
    data_file: &logiqx::DataFile,
    data_file_name: &str,
) -> i32 {
    use schema::{data_files, data_files::dsl::*};

    let results = data_files
        .filter(name.eq(data_file.header().name()))
        .select(id)
        .first::<i32>(conn)
        .optional()
        .expect("Somehow unable to query for data_files");

    // I actually want upsert. there's got to be an easier way
    if let Some(data_file_id) = results {
        data_file_id
    } else {
        // this should be some kind of conversion in data_file.rs
        let new_data_file = NewDataFile {
            build: data_file.build(),
            debug: data_file.debug(),
            file_name: data_file_name,
            name: data_file.header().name(),
            description: data_file.header().description(),
            version: data_file.header().version(),
            author: data_file.header().author(),
            homepage: data_file.header().homepage(),
            url: data_file.header().url(),
        };

        diesel::insert_into(data_files::table)
            .values(&new_data_file)
            .execute(conn)
            .expect("Error saving datfile")
            .try_into()
            .unwrap()
    }
}
