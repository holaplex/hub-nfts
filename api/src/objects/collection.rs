use async_graphql::{Context, Error, Object, Result};
use sea_orm::entity::prelude::*;

use super::{metadata_json::MetadataJson, CollectionMint, Drop, Holder};
use crate::{
    entities::{
        collection_creators,
        collections::Model,
        mint_histories,
        sea_orm_active_enums::{Blockchain, CreationStatus},
    },
    AppContext,
};

/// An NFT collection that has either a fixed supply or unlimited mints. NFT collections are deployed to a desired blockchain.
/// On Solana, when the collection is associated to a drop it is a `master_edition`. When the collection is not associated to a drop it is a sized Metaplex certified collection.
/// On EVM chains, the collection is a ERC-1155 token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collection {
    /// The unique identifier for the collection.
    pub id: Uuid,
    /// The blockchain of the collection.
    pub blockchain: Blockchain,
    /// The creation status of the collection. When the collection is in a `CREATED` status you can mint NFTs from the collection.
    pub creation_status: CreationStatus,
    /// The blockchain address of the collection used to view it in blockchain explorers.
    /// On Solana this is the mint address.
    /// On EVM chains it is the concatenation of the contract address and the token id `{contractAddress}:{tokenId}`.
    pub address: Option<String>,
    /// The transaction signature of the collection.
    pub signature: Option<String>,
    /// The royalties assigned to mints belonging to the collection expressed in basis points.
    pub seller_fee_basis_points: i16,
    /// The project id of the collection.
    pub project_id: Uuid,
    /// The credits deduction id of the collection.
    pub credits_deduction_id: Option<Uuid>,
    /// The date and time in UTC when the collection was created.
    pub created_at: DateTimeWithTimeZone,
    /// The user id of the person who created the collection.
    pub created_by: Uuid,
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
    async fn supply(&self, ctx: &Context<'_>) -> Result<Option<i64>> {
        let AppContext {
            collection_supply_loader,
            ..
        } = ctx.data::<AppContext>()?;

        let supply = collection_supply_loader
            .load_one(self.id)
            .await?
            .ok_or(Error::new("Unable to find collection supply"))?;

        Ok(supply)
    }

    /// The creation status of the collection. When the collection is in a `CREATED` status you can mint NFTs from the collection.
    async fn creation_status(&self) -> CreationStatus {
        self.creation_status
    }

    async fn project_id(&self) -> Uuid {
        self.project_id
    }

    /// The date and time in UTC when the collection was created.
    async fn created_at(&self) -> DateTimeWithTimeZone {
        self.created_at
    }

    /// The user id of the person who created the collection.
    async fn created_by_id(&self) -> Uuid {
        self.created_by
    }

    async fn credits_deduction_id(&self) -> Option<Uuid> {
        self.credits_deduction_id
    }

    /// The blockchain address of the collection used to view it in blockchain explorers.
    /// On Solana this is the mint address.
    /// On EVM chains it is the concatenation of the contract address and the token id `{contractAddress}:{tokenId}`.
    async fn address(&self) -> Option<String> {
        self.address.clone()
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
    async fn mints(&self, ctx: &Context<'_>) -> Result<Option<Vec<CollectionMint>>> {
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

    #[graphql(deprecation = "Use `mint_histories` instead")]
    /// A list of all NFT purchases from the collection, including both primary and secondary sales.
    async fn purchases(&self, ctx: &Context<'_>) -> Result<Option<Vec<mint_histories::Model>>> {
        let AppContext {
            collection_mint_history_loader,
            ..
        } = ctx.data::<AppContext>()?;

        collection_mint_history_loader.load_one(self.id).await
    }

    /// A list of all NFT mints from the collection, including both primary and secondary sales.
    async fn mint_histories(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<Vec<mint_histories::Model>>> {
        let AppContext {
            collection_mint_history_loader,
            ..
        } = ctx.data::<AppContext>()?;

        collection_mint_history_loader.load_one(self.id).await
    }

    async fn drop(&self, ctx: &Context<'_>) -> Result<Option<Drop>> {
        let AppContext {
            collection_drop_loader,
            ..
        } = ctx.data::<AppContext>()?;

        collection_drop_loader.load_one(self.id).await
    }

    /// The current number of NFTs minted from the collection.
    async fn total_mints(&self, ctx: &Context<'_>) -> Result<i64> {
        let AppContext {
            collection_total_mints_loader,
            ..
        } = ctx.data::<AppContext>()?;

        let total_mints = collection_total_mints_loader
            .load_one(self.id)
            .await?
            .ok_or(Error::new("Unable to find collection total mints"))?;

        Ok(total_mints)
    }
}

impl From<Model> for Collection {
    fn from(
        Model {
            id,
            blockchain,
            creation_status,
            signature,
            seller_fee_basis_points,
            address,
            project_id,
            credits_deduction_id,
            created_at,
            created_by,
            ..
        }: Model,
    ) -> Self {
        Self {
            id,
            blockchain,
            creation_status,
            address,
            signature,
            seller_fee_basis_points,
            project_id,
            credits_deduction_id,
            created_at,
            created_by,
        }
    }
}
