use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use hub_core::uuid::Uuid;

use crate::{
    objects::{Collection, Drop},
    AppContext,
};

#[derive(SimpleObject, Debug, Clone)]
#[graphql(complex)]
pub struct Project {
    pub id: Uuid,
}

#[ComplexObject]
impl Project {
    /// The drops associated with the project.
    async fn drops(&self, ctx: &Context<'_>) -> Result<Option<Vec<Drop>>> {
        let AppContext {
            project_drops_loader,
            ..
        } = ctx.data::<AppContext>()?;

        project_drops_loader.load_one(self.id).await
    }

    /// Look up a drop associated with the project by its ID.
    async fn drop(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<Drop>> {
        let AppContext { drop_loader, .. } = ctx.data::<AppContext>()?;

        drop_loader.load_one(id).await
    }

    /// The collections associated with the project.
    async fn collections(&self, ctx: &Context<'_>) -> Result<Option<Vec<Collection>>> {
        let AppContext {
            project_collections_loader,
            ..
        } = ctx.data::<AppContext>()?;

        project_collections_loader.load_one(self.id).await
    }

    async fn collection(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<Collection>> {
        let AppContext {
            project_collection_loader,
            ..
        } = ctx.data::<AppContext>()?;

        project_collection_loader.load_one(id).await
    }
}
