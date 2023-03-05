use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use hub_core::uuid::Uuid;

use crate::{entities::drops, AppContext};

#[derive(SimpleObject, Debug, Clone)]
#[graphql(complex)]
pub struct Project {
    pub id: Uuid,
}

#[ComplexObject]
impl Project {
    async fn drops(&self, ctx: &Context<'_>) -> Result<Option<Vec<drops::Model>>> {
        let AppContext {
            project_drops_loader,
            ..
        } = ctx.data::<AppContext>()?;

        project_drops_loader.load_one(self.id).await
    }

    async fn drop(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<drops::Model>> {
        let AppContext { drop_loader, .. } = ctx.data::<AppContext>()?;

        drop_loader.load_one(id).await
    }
}
