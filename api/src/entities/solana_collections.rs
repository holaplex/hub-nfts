//! `SeaORM` Entity. Generated by sea-orm-codegen 0.10.5

use async_graphql::SimpleObject;
use sea_orm::entity::prelude::*;

use super::sea_orm_active_enums::CreationStatus;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, SimpleObject)]
#[sea_orm(table_name = "solana_collections")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub project_id: Uuid,
    pub address: String,
    pub update_authority: String,
    pub ata_pubkey: String,
    pub owner_pubkey: String,
    pub mint_pubkey: String,
    pub metadata_pubkey: String,
    pub name: String,
    pub description: String,
    pub metadata_uri: String,
    pub animation_uri: Option<String>,
    pub image_uri: String,
    pub external_link: Option<String>,
    pub seller_fee_basis_points: i16,
    pub royalty_wallet: String,
    pub supply: Option<i64>,
    pub creation_status: CreationStatus,
    pub created_by: Uuid,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
