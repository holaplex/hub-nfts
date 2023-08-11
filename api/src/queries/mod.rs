#![allow(clippy::unused_async)]

mod collection;
mod customer;
mod drop;
mod mint;
mod project;
mod wallet;

// // Add your other ones here to create a unified Query object
#[derive(async_graphql::MergedObject, Default)]
pub struct Query(
    project::Query,
    wallet::Query,
    customer::Query,
    mint::Query,
    collection::Query,
    drop::Query,
);
