use async_graphql::{EmptySubscription, Schema, extensions};
use holaplex_rust_boilerplate_core::prelude::*;

use crate::{
    db::Connection,
    graphql::{mutation::Mutation, query::Query},
};

pub type AppSchema = Schema<Query, Mutation, EmptySubscription>;

/// Builds the GraphQL Schema, attaching the Database to the context
pub async fn build_schema() -> Result<AppSchema> {
    let db = Connection::new()
        .await
        .context("failed to get db connection")?;

    // todo! Shared struct instead of db

    let schema = Schema::build(Query::default(), Mutation::default(), EmptySubscription)
        .extension(extensions::Logger)
        .data(db.get())
        .finish();

    Ok(schema)
}