use async_graphql::{Context, Object, Result};
use hub_core::uuid::Uuid;

use crate::{objects::Drop, AppContext};

#[derive(Debug, Clone, Copy, Default)]
pub struct Query;

#[Object(name = "DropQuery")]
impl Query {
    /// Look up a `drop` by its ID.
    async fn drop(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<Drop>> {
        let AppContext { drop_loader, .. } = ctx.data::<AppContext>()?;

        drop_loader.load_one(id).await
    }
}
