use async_graphql::{ComplexObject, Context, Error, Result, SimpleObject};
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
    #[graphql(deprecation = "Use `drop` root query field instead")]
    async fn drop(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<Drop>> {
        let AppContext { drop_loader, .. } = ctx.data::<AppContext>()?;

        let drop = drop_loader.load_one(id).await?;

        if let Some(drop) = drop {
            if drop.project_id == self.id {
                return Ok(Some(drop));
            }

            return Err(Error::new(format!(
                "Drop {} is not associated with project {}",
                id, self.id
            )));
        }

        Ok(None)
    }

    /// The collections associated with the project.
    async fn collections(&self, ctx: &Context<'_>) -> Result<Option<Vec<Collection>>> {
        let AppContext {
            project_collections_loader,
            ..
        } = ctx.data::<AppContext>()?;

        project_collections_loader.load_one(self.id).await
    }

    /// Look up a collection associated with the project by its ID.
    #[graphql(deprecation = "Use `collection` root query field instead")]
    async fn collection(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<Collection>> {
        let AppContext {
            project_collection_loader,
            ..
        } = ctx.data::<AppContext>()?;

        let collection = project_collection_loader.load_one(id).await?;

        if let Some(collection) = collection {
            if collection.project_id == self.id {
                return Ok(Some(collection));
            }

            return Err(Error::new(format!(
                "Collection {} is not associated with project {}",
                id, self.id
            )));
        }

        Ok(None)
    }
}
