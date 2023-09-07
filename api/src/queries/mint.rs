use async_graphql::{Context, Object, Result};
use hub_core::uuid::Uuid;

use crate::{objects::CollectionMint, AppContext};

#[derive(Debug, Clone, Copy, Default)]
pub struct Query;

#[Object(name = "MintQuery")]
impl Query {
    /// Look up a `collection_mint` by its ID.
    async fn mint(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<CollectionMint>> {
        let AppContext {
            single_collection_mint_loader,
            ..
        } = ctx.data::<AppContext>()?;

        single_collection_mint_loader.load_one(id).await
    }
}
