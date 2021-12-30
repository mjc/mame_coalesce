use std::convert::TryInto;

use diesel::SqliteConnection;

use crate::models::*;
use crate::{logiqx, models};

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
) -> usize {
    use schema::{data_files, data_files::dsl::*};

    let new_data_file = (
        build.eq(data_file.build()),
        debug.eq(data_file.debug()),
        file_name.eq(data_file_name),
        name.eq(data_file.header().name()),
        description.eq(data_file.header().description()),
        version.eq(data_file.header().version()),
        author.eq(data_file.header().author()),
        homepage.eq(data_file.header().homepage()),
        url.eq(data_file.header().url()),
    );

    let insert_id = diesel::insert_into(data_files::table)
        .values(&new_data_file)
        .execute(conn)
        .optional()
        .unwrap_or(None);

    match insert_id {
        Some(data_file_id) => data_file_id,
        None => diesel::update(data_files.filter(name.eq(data_file.header().name())))
            .set(new_data_file)
            .execute(conn)
            .expect("Error updating DataFile"),
    }
}
