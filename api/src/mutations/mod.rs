pub mod drop;

// // Add your other ones here to create a unified Mutation object
// // e.x. Mutation(OrganizationMutation, OtherMutation, OtherOtherMutation)
#[derive(async_graphql::MergedObject, Default)]
pub struct Mutation(drop::Mutation);
