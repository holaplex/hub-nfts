use async_graphql::{Context, Object, Result};
use hub_core::uuid::Uuid;

use crate::{objects::Collection, AppContext};

#[derive(Debug, Clone, Copy, Default)]
pub struct Query;

#[Object(name = "CollectionQuery")]
impl Query {
    /// Look up a `collection` by its ID.
    async fn collection(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<Collection>> {
        let AppContext {
            single_project_collection_loader,
            ..
        } = ctx.data::<AppContext>()?;

        single_project_collection_loader.load_one(id).await
    }
}
