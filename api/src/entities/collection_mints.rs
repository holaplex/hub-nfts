//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.0

use async_graphql::{ComplexObject, Context, Error, Result, SimpleObject};
use sea_orm::entity::prelude::*;

use super::sea_orm_active_enums::{Blockchain, CreationStatus};
use crate::{
    objects::{Collection, MetadataJson},
    AppContext,
};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "collection_mints")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub collection_id: Uuid,
    #[sea_orm(column_type = "Text", nullable)]
    pub address: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub owner: String,
    pub creation_status: CreationStatus,
    pub created_by: Uuid,
    pub created_at: DateTimeWithTimeZone,
    #[sea_orm(nullable)]
    pub signature: Option<String>,
    pub edition: i64,
    pub seller_fee_basis_points: i16,
    pub credits_deduction_id: Option<Uuid>,
}

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
    pub owner: String,
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
        }
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::collections::Entity",
        from = "Column::CollectionId",
        to = "super::collections::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Collections,
    #[sea_orm(has_many = "super::purchases::Entity")]
    Purchases,
}

impl Related<super::collections::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Collections.def()
    }
}

impl Related<super::purchases::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Purchases.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
