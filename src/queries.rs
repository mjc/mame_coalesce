use std::path::Path;

use sqlx::SqlitePool;

use super::logiqx::DataFile;

// this definitely should not be one giant file

pub async fn insert_data_file(pool: SqlitePool, data_file: DataFile, path: &str) {
    // gross
    let mut conn = pool.acquire().await.unwrap();
    // gross
    let file_name = Path::new(path).file_name().unwrap().to_str().unwrap();

    sqlx::query!(
        r#"
        INSERT INTO data_files (
            build,
            debug,
            name,
            description,
            version,
            author,
            homepage,
            url,
            file_name
        )
        VALUES ( ?, ?, ?, ?, ?, ?, ?, ?, ? )
        ON CONFLICT(name)
            DO UPDATE SET
                build = excluded.build,
                debug = excluded.debug,
                description = excluded.description,
                version = excluded.version,
                author = excluded.author,
                homepage = excluded.homepage,
                url = excluded.url,
                file_name = excluded.file_name
         "#,
        data_file.build,
        data_file.debug,
        data_file.header().name,
        data_file.header().description,
        data_file.header().version,
        data_file.header().author,
        data_file.header().homepage,
        data_file.header().url,
        file_name
    )
    .execute(&mut conn)
    .await
    .unwrap();
}

// build       STRING  NOT NULL,
// debug       BOOLEAN,
// file_name   STRING,
// name        STRING,
// description STRING,
// category    STRING,
// version     STRING,
// author      STRING,
// email       STRING,
// homepage    STRING,
// url         STRING
