use async_graphql::{Context, Object, Result};
use sea_orm::entity::prelude::*;

use super::{metadata_json::MetadataJson, Holder};
use crate::{
    entities::{
        collection_creators, collection_mints,
        collections::Model,
        purchases,
        sea_orm_active_enums::{Blockchain, CreationStatus},
    },
    AppContext,
};

/// An NFT collection that has either a fixed supply or unlimited mints. NFT collections are deployed to a desired blockchain.
#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// On Solana this is the mint address.
    /// On EVM chains it is the concatenation of the contract address and the token id `{contractAddress}:{tokenId}`.
    pub address: Option<String>,
    /// The current number of NFTs minted from the collection.
    pub total_mints: i64,
    /// The transaction signature of the collection.
    pub signature: Option<String>,
    /// The royalties assigned to mints belonging to the collection expressed in basis points.
    pub seller_fee_basis_points: i16,
    pub project_id: Uuid,
    pub credits_deduction_id: Option<Uuid>,
}

#[Object]
impl Collection {
    /// The unique identifier for the collection.
    async fn id(&self) -> Uuid {
        self.id
    }

    /// The blockchain of the collection.
    async fn blockchain(&self) -> Blockchain {
        self.blockchain
    }
    /// The total supply of the collection. Setting to `null` implies unlimited minting.
    async fn supply(&self) -> Option<i64> {
        self.supply
    }

    /// The creation status of the collection. When the collection is in a `CREATED` status you can mint NFTs from the collection.
    async fn creation_status(&self) -> CreationStatus {
        self.creation_status
    }

    async fn project_id(&self) -> Uuid {
        self.project_id
    }

    async fn credits_deduction_id(&self) -> Option<Uuid> {
        self.credits_deduction_id.clone()
    }

    /// The blockchain address of the collection used to view it in blockchain explorers.
    /// On Solana this is the mint address.
    /// On EVM chains it is the concatenation of the contract address and the token id `{contractAddress}:{tokenId}`.
    async fn address(&self) -> Option<String> {
        self.address.clone()
    }

    /// The current number of NFTs minted from the collection.
    async fn total_mints(&self) -> i64 {
        self.total_mints
    }

    /// The transaction signature of the collection.
    async fn signature(&self) -> Option<String> {
        self.signature.clone()
    }

    /// The royalties assigned to mints belonging to the collection expressed in basis points.
    async fn seller_fee_basis_points(&self) -> i16 {
        self.seller_fee_basis_points
    }

    /// The metadata json associated to the collection.
    /// ## References
    /// [Metaplex v1.1.0 Standard](https://docs.metaplex.com/programs/token-metadata/token-standard)
    async fn metadata_json(&self, ctx: &Context<'_>) -> Result<Option<MetadataJson>> {
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

    /// A list of all NFT purchases from the collection, including both primary and secondary sales.
    async fn purchases(&self, ctx: &Context<'_>) -> Result<Option<Vec<purchases::Model>>> {
        let AppContext {
            collection_purchases_loader,
            ..
        } = ctx.data::<AppContext>()?;

        collection_purchases_loader.load_one(self.id).await
    }
}

impl From<Model> for Collection {
    fn from(
        Model {
            id,
            blockchain,
            supply,
            creation_status,
            total_mints,
            signature,
            seller_fee_basis_points,
            address,
            project_id,
            credits_deduction_id,
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
            seller_fee_basis_points,
            project_id,
            credits_deduction_id,
        }
    }
}
