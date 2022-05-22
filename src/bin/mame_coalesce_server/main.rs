use axum::{extract::Extension, response::Html, routing::post, Router};

use mame_coalesce::{db, logiqx, MameResult};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> MameResult<()> {
    let pool = db::create_async_pool();

    // build our application with a route
    let app = Router::new()
        .route("/datfile", post(handler))
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

async fn handler(body: String, Extension(pool): Extension<db::AsyncPool>) -> Html<&'static str> {
    let datfile = logiqx::DataFile::from_string(&body).expect("Couldn't parse datfile");
    let managed_conn = pool.get().await.expect("Couldn't check out db connection");
    let _id = managed_conn
        .interact(move |conn| {
            db::traverse_and_insert_data_file(conn, &datfile).expect("Couldn't insert datfile")
        })
        .await;

    Html("moo")
}
