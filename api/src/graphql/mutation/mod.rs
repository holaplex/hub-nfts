// Add your other ones here to create a unified Mutation object
// e.x. Mutation(SomeMutation, OtherMutation, OtherOtherMutation)
#[derive(async_graphql::MergedObject, Default)]
pub struct Mutation;