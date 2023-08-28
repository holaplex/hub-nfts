//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.3

use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use sea_orm::entity::prelude::*;

use super::sea_orm_active_enums::CreationStatus;
use crate::{objects::CollectionMint, AppContext};

/// A record of a minted NFT.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, SimpleObject)]
#[sea_orm(table_name = "mint_histories")]
#[graphql(concrete(name = "MintHistory", params()), complex)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// The ID of the NFT minted.
    pub mint_id: Uuid,
    /// The wallet address of the buyer.
    #[sea_orm(column_type = "Text")]
    pub wallet: String,
    /// The signature of the transaction, if any.
    #[sea_orm(column_type = "Text", nullable)]
    pub tx_signature: Option<String>,
    /// The status of the creation of the NFT.
    pub status: CreationStatus,
    /// The date and time when the purchase was created.
    pub created_at: DateTimeWithTimeZone,
    /// The ID of the collection that facilitated the mint, if any.
    pub collection_id: Uuid,
}

impl ActiveModelBehavior for ActiveModel {
    hub_core::before_save_evm_addrs!(wallet);
}

#[ComplexObject]
impl Model {
    /// The minted NFT.
    async fn mint(&self, ctx: &Context<'_>) -> Result<Option<CollectionMint>> {
        let AppContext {
            single_collection_mint_loader,
            ..
        } = ctx.data::<AppContext>()?;

        single_collection_mint_loader.load_one(self.mint_id).await
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::collection_mints::Entity",
        from = "Column::MintId",
        to = "super::collection_mints::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    CollectionMints,
    #[sea_orm(
        belongs_to = "super::collections::Entity",
        from = "Column::CollectionId",
        to = "super::collections::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Collections,
}

impl Related<super::collection_mints::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CollectionMints.def()
    }
}

impl Related<super::collections::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Collections.def()
    }
}
