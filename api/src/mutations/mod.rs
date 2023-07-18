#![allow(clippy::too_many_lines)]
#![allow(clippy::unused_async)]
pub mod collection;
pub mod drop;
pub mod mint;
pub mod transfer;

// // Add your other ones here to create a unified Mutation object
// // e.x. Mutation(OrganizationMutation, OtherMutation, OtherOtherMutation)
#[derive(async_graphql::MergedObject, Default)]
pub struct Mutation(
    collection::Mutation,
    mint::Mutation,
    transfer::Mutation,
    drop::Mutation,
);
