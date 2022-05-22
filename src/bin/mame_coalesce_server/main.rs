use axum::{extract::Extension, response::Html, routing::post, Router};
use deadpool_diesel::sqlite::{Manager, Pool, Runtime};

use mame_coalesce::{db, logiqx, MameResult};
use std::net::SocketAddr;

type AsyncPool = deadpool::managed::Pool<deadpool_diesel::Manager<diesel::SqliteConnection>>;

#[tokio::main]
async fn main() -> MameResult<()> {
    // build our application with a route
    let manager = Manager::new("coalesce.db", Runtime::Tokio1);
    let pool: AsyncPool = Pool::builder(manager).max_size(8).build().unwrap();

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

async fn handler(body: String, Extension(pool): Extension<AsyncPool>) -> Html<&'static str> {
    let datfile = logiqx::DataFile::from_string(&body).expect("Couldn't parse datfile");
    let managed_conn = pool.get().await.expect("Couldn't check out db connection");
    let _id = managed_conn
        .interact(move |conn| {
            db::traverse_and_insert_data_file(conn, &datfile).expect("Couldn't insert datfile")
        })
        .await;

    Html("moo")
}
