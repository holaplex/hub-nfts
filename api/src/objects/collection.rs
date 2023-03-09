use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use sea_orm::entity::prelude::*;

use super::Holder;
use crate::{
    entities::{
        collection_creators, collection_mints,
        collections::Model,
        metadata_jsons,
        sea_orm_active_enums::{Blockchain, CreationStatus},
    },
    AppContext,
};

#[derive(Clone, Debug, PartialEq, Eq, SimpleObject)]
#[graphql(complex)]
pub struct Collection {
    pub id: Uuid,
    pub blockchain: Blockchain,
    pub supply: Option<i64>,
    pub creation_status: CreationStatus,
    pub address: Option<String>,
    pub total_mints: i64,
}

#[ComplexObject]
impl Collection {
    async fn metadata_json(&self, ctx: &Context<'_>) -> Result<Option<metadata_jsons::Model>> {
        let AppContext {
            metadata_json_loader,
            ..
        } = ctx.data::<AppContext>()?;

        metadata_json_loader.load_one(self.id).await
    }

    async fn mints(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<Vec<collection_mints::CollectionMint>>> {
        let AppContext {
            collection_mints_loader,
            ..
        } = ctx.data::<AppContext>()?;

        collection_mints_loader.load_one(self.id).await
    }

    async fn creators(&self, ctx: &Context<'_>) -> Result<Option<Vec<collection_creators::Model>>> {
        let AppContext {
            creators_loader, ..
        } = ctx.data::<AppContext>()?;

        creators_loader.load_one(self.id).await
    }

    async fn holders(&self, ctx: &Context<'_>) -> Result<Option<Vec<Holder>>> {
        let AppContext { holders_loader, .. } = ctx.data::<AppContext>()?;

        holders_loader.load_one(self.id).await
    }
}

impl From<Model> for Collection {
    fn from(
        Model {
            id,
            blockchain,
            supply,
            creation_status,
            address,
            total_mints,
        }: Model,
    ) -> Self {
        Self {
            id,
            blockchain,
            supply,
            creation_status,
            address,
            total_mints,
        }
    }
}
