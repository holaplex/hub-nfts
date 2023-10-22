use async_graphql::{ComplexObject, Context, Error, Result, SimpleObject};
use sea_orm::entity::prelude::*;

use crate::{
    entities::{
        collection_mints::Model,
        mint_creators, mint_histories, nft_transfers,
        sea_orm_active_enums::{Blockchain, CreationStatus},
        switch_collection_histories, update_histories,
    },
    objects::{Collection, MetadataJson},
    AppContext,
};

/// Represents a single NFT minted from a collection.
#[derive(Clone, Debug, PartialEq, Eq, SimpleObject)]
#[graphql(complex)]
pub struct CollectionMint {
    /// The unique ID of the minted NFT.
    pub id: Uuid,
    /// The ID of the collection the NFT was minted from.
    pub collection_id: Uuid,
    /// The address of the NFT
    /// On Solana this is the mint address.
    /// On EVM chains it is the concatenation of the contract address and the token id `{contractAddress}:{tokenId}`.
    pub address: Option<String>,
    /// The wallet address of the owner of the NFT.
    pub owner: Option<String>,
    /// The status of the NFT creation.
    pub creation_status: CreationStatus,
    /// The unique ID of the creator of the NFT.
    pub created_by: Uuid,
    /// The date and time when the NFT was created.
    pub created_at: DateTimeWithTimeZone,
    /// The transaction signature associated with the NFT.
    pub signature: Option<String>,
    /// The unique edition number of the NFT.
    pub edition: i64,
    /// The seller fee basis points (ie royalties) for the NFT.
    pub seller_fee_basis_points: i16,
    /// credits deduction id
    pub credits_deduction_id: Option<Uuid>,
    /// Indicates if the NFT is compressed. Compression is only supported on Solana.
    pub compressed: Option<bool>,
}

#[ComplexObject]
impl CollectionMint {
    /// The collection the NFT was minted from.
    async fn collection(&self, ctx: &Context<'_>) -> Result<Option<Collection>> {
        let AppContext {
            collection_loader, ..
        } = ctx.data::<AppContext>()?;

        collection_loader.load_one(self.collection_id).await
    }

    /// The metadata json associated to the collection.
    /// [Metaplex v1.1.0 Standard](https://docs.metaplex.com/programs/token-metadata/token-standard)
    async fn metadata_json(&self, ctx: &Context<'_>) -> Result<Option<MetadataJson>> {
        let AppContext {
            metadata_json_loader,
            ..
        } = ctx.data::<AppContext>()?;
        let collection = self.collection(ctx).await?.ok_or(Error::new(format!(
            "Collection not found for collection mint {:?}",
            &self.id
        )))?;

        match collection.blockchain {
            Blockchain::Solana => metadata_json_loader.load_one(self.id).await,
            Blockchain::Polygon => metadata_json_loader.load_one(self.collection_id).await,
            Blockchain::Ethereum => Err(Error::new("Ethereum not supported")),
        }
    }

    /// The update history of the mint.
    async fn update_histories(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<Vec<update_histories::Model>>> {
        let AppContext {
            update_mint_history_loader,
            ..
        } = ctx.data::<AppContext>()?;

        update_mint_history_loader.load_one(self.id).await
    }
    /// The creators of the mint. Includes the creator addresses and their shares.
    async fn creators(&self, ctx: &Context<'_>) -> Result<Option<Vec<mint_creators::Model>>> {
        let AppContext {
            mint_creators_loader,
            ..
        } = ctx.data::<AppContext>()?;

        mint_creators_loader.load_one(self.id).await
    }

    /// The record of the original mint.
    async fn mint_history(&self, ctx: &Context<'_>) -> Result<Option<mint_histories::Model>> {
        let AppContext {
            collection_mint_mint_history_loader,
            ..
        } = ctx.data::<AppContext>()?;

        collection_mint_mint_history_loader.load_one(self.id).await
    }

    /// The history of transfers for the mint.
    async fn transfer_histories(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<Vec<nft_transfers::Model>>> {
        let AppContext {
            collection_mint_transfers_loader,
            ..
        } = ctx.data::<AppContext>()?;

        collection_mint_transfers_loader.load_one(self.id).await
    }

    /// The history of switched collections for the mint.
    async fn switch_collection_histories(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<Vec<switch_collection_histories::Model>>> {
        let AppContext {
            switch_collection_history_loader,
            ..
        } = ctx.data::<AppContext>()?;

        switch_collection_history_loader.load_one(self.id).await
    }
}

impl From<Model> for CollectionMint {
    fn from(
        Model {
            id,
            collection_id,
            address,
            owner,
            creation_status,
            created_by,
            created_at,
            signature,
            edition,
            seller_fee_basis_points,
            credits_deduction_id,
            compressed,
        }: Model,
    ) -> Self {
        Self {
            id,
            collection_id,
            address,
            owner,
            creation_status,
            created_by,
            created_at,
            signature,
            edition,
            seller_fee_basis_points,
            credits_deduction_id,
            compressed,
        }
    }
}
