//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.0

use sea_orm::entity::prelude::*;

use super::sea_orm_active_enums::{Blockchain, CreationStatus};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "collections")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub blockchain: Blockchain,
    pub supply: Option<i64>,
    pub project_id: Uuid,
    #[sea_orm(nullable)]
    pub credits_deduction_id: Option<Uuid>,
    pub creation_status: CreationStatus,
    pub total_mints: i64,
    #[sea_orm(column_type = "Text", nullable)]
    pub address: Option<String>,
    #[sea_orm(nullable)]
    pub signature: Option<String>,
    pub seller_fee_basis_points: i16,
    pub created_by: Uuid,
    pub created_at: DateTimeWithTimeZone,
}

impl ActiveModelBehavior for ActiveModel {
    hub_core::before_save_evm_addrs!(address?);
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::collection_creators::Entity")]
    CollectionCreators,
    #[sea_orm(has_many = "super::collection_mints::Entity")]
    CollectionMints,
    #[sea_orm(has_one = "super::drops::Entity")]
    Drop,
    #[sea_orm(has_many = "super::mint_histories::Entity")]
    MintHistories,
}

impl Related<super::collection_creators::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CollectionCreators.def()
    }
}

impl Related<super::collection_mints::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CollectionMints.def()
    }
}

impl Related<super::drops::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Drop.def()
    }
}

impl Related<super::mint_histories::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MintHistories.def()
    }
}
