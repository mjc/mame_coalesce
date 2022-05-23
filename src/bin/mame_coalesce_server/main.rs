use axum::{extract::Extension, response::Html, routing::post, Json, Router};

use hyper::StatusCode;
use mame_coalesce::{db, logiqx, MameResult};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> MameResult<()> {
    let pool = db::create_sync_pool("coalesce.db")?;

    // build our application with a route
    let app = Router::new()
        .route("/datfile", post(add_datfile))
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
    let datfile =
        logiqx::DataFile::from_string(&body).or_else(|_| Err(StatusCode::UNPROCESSABLE_ENTITY))?;
    db::traverse_and_insert_data_file(
        &mut pool
            .get()
            .or_else(|_| Err(StatusCode::INTERNAL_SERVER_ERROR))?,
        &datfile,
    )
    .expect("Couldn't insert datfile");
    Ok(Json("moo"))
}
