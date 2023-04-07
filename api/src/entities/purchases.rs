//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.0

use async_graphql::SimpleObject;
use sea_orm::entity::prelude::*;

use super::sea_orm_active_enums::CreationStatus;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, SimpleObject)]
#[sea_orm(table_name = "purchases")]
#[graphql(concrete(name = "Purchase", params()))]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub mint_id: Uuid,
    #[sea_orm(column_type = "Text")]
    pub wallet: String,
    pub spent: i64,
    #[sea_orm(column_type = "Text", nullable)]
    pub tx_signature: Option<String>,
    pub status: CreationStatus,
    pub created_at: DateTime,
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
}

impl Related<super::collection_mints::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CollectionMints.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
