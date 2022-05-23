use axum::{extract::Extension, routing::post, Json, Router};

use hyper::StatusCode;
use log::info;
use mame_coalesce::{build_rayon_pool, db, logger, logiqx, operations::scan, MameResult};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> MameResult<()> {
    logger::setup_logger();

    let pool = db::create_sync_pool("coalesce.db")?;

    build_rayon_pool()?;

    // build our application with a route
    let app = Router::new()
        .route("/datfile", post(add_datfile))
        .route("/scan_source", post(scan_source))
        .layer(Extension(pool));

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
    Ok(())
}

async fn add_datfile(
    body: String,
    Extension(pool): Extension<db::SyncPool>,
) -> Result<Json<&'static str>, hyper::StatusCode> {
    let conn = &mut pool
        .get()
        .or_else(|_| Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let datfile =
        logiqx::DataFile::from_string(&body).or_else(|_| Err(StatusCode::UNPROCESSABLE_ENTITY))?;
    db::traverse_and_insert_data_file(conn, &datfile).expect("Couldn't insert datfile");
    Ok(Json("moo"))
}

async fn scan_source(
    path: String,
    Extension(pool): Extension<db::SyncPool>,
) -> Result<Json<&'static str>, hyper::StatusCode> {
    let conn = &mut pool.get().or_else(|err| {
        log::error!("database connection error: {:?}", err);
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    })?;
    let file_list = scan::walk_for_files(camino::Utf8Path::new(path.as_str()));
    let new_rom_files = scan::get_all_rom_files(&file_list).or_else(|err| {
        log::error!("failed to get all rom files: {:?}", err);
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    })?;

    info!(
        "rom files found (unpacked and packed both): {}",
        new_rom_files.len()
    );
    db::import_rom_files(conn, &new_rom_files).or_else(|err| {
        log::error!("failed to import rom files: {:?}", err);
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    })?;
    Ok(Json("moo"))
}
