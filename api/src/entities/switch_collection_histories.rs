//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.3
use async_graphql::SimpleObject;
use sea_orm::entity::prelude::*;

use super::sea_orm_active_enums::CreationStatus;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, SimpleObject)]
#[sea_orm(table_name = "switch_collection_histories")]
#[graphql(concrete(name = "SwitchCollectionHistory", params()))]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub collection_mint_id: Uuid,
    pub collection_id: Uuid,
    pub credit_deduction_id: Uuid,
    #[sea_orm(column_type = "Text", nullable)]
    pub signature: Option<String>,
    pub status: CreationStatus,
    pub initiated_by: Uuid,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}