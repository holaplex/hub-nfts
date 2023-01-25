mod db;
mod graphql;

use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql_poem::GraphQL;
use holaplex_rust_boilerplate_core::prelude::*;
use poem::{get, handler, listener::TcpListener, post, web::Html, IntoResponse, Route, Server};

use crate::graphql::schema::build_schema;

#[handler]
async fn playground() -> impl IntoResponse {
    Html(playground_source(GraphQLPlaygroundConfig::new("/graphql")))
}

#[tokio::main]
pub async fn start() -> Result<()> {
    if cfg!(debug_assertions) {
        dotenv::dotenv().ok();
    }

    env_logger::builder()
        .filter_level(if cfg!(debug_assertions) {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        .parse_default_env()
        .init();

    let schema = build_schema().await?;

    Server::new(TcpListener::bind("127.0.0.1:3001"))
        .run(
            Route::new()
                .at("/graphql", post(GraphQL::new(schema)))
                .at("/playground", get(playground)),
        )
        .await
        .map_err(Into::into)
}
