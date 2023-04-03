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

/// An NFT collection that has either a fixed supply or unlimited mints. NFT collections are deployed to a desired blockchain.
#[derive(Clone, Debug, PartialEq, Eq, SimpleObject)]
#[graphql(complex)]
pub struct Collection {
    /// The unique identifier for the collection.
    pub id: Uuid,
    /// The blockchain of the collection.
    pub blockchain: Blockchain,
    /// The total supply of the collection. Setting to `null` implies unlimited minting.
    pub supply: Option<i64>,
    /// The creation status of the collection. When the collection is in a `CREATED` status you can mint NFTs from the collection.
    pub creation_status: CreationStatus,
    /// The blockchain address of the collection used to view it in blockchain explorers.
    pub address: Option<String>,
    /// The current number of NFTs minted from the collection.
    pub total_mints: i64,
    pub signature: Option<String>,
}

#[ComplexObject]
impl Collection {
    /// The metadata json associated to the collection.
    /// ## References
    /// [Metaplex v1.1.0 Standard](https://docs.metaplex.com/programs/token-metadata/token-standard)
    async fn metadata_json(&self, ctx: &Context<'_>) -> Result<Option<metadata_jsons::Model>> {
        let AppContext {
            metadata_json_loader,
            ..
        } = ctx.data::<AppContext>()?;

        metadata_json_loader.load_one(self.id).await
    }

    /// The list of minted NFTs from the collection including the NFTs address and current owner's wallet address.
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

    /// The list of attributed creators for the collection.
    async fn creators(&self, ctx: &Context<'_>) -> Result<Option<Vec<collection_creators::Model>>> {
        let AppContext {
            creators_loader, ..
        } = ctx.data::<AppContext>()?;

        creators_loader.load_one(self.id).await
    }

    /// The list of current holders of NFTs from the collection.
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
            signature,
        }: Model,
    ) -> Self {
        Self {
            id,
            blockchain,
            supply,
            creation_status,
            address,
            total_mints,
            signature,
        }
    }
}
