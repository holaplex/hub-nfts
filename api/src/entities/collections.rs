//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.0

use async_graphql::SimpleObject;
use sea_orm::entity::prelude::*;

use super::sea_orm_active_enums::{Blockchain, CreationStatus};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, SimpleObject)]
#[sea_orm(table_name = "collections")]
#[graphql(concrete(name = "Collection", params()))]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub blockchain: Blockchain,
    pub name: String,
    pub description: String,
    pub metadata_uri: String,
    pub royalty_wallet: String,
    pub supply: Option<i64>,
    pub creation_status: CreationStatus,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::drops::Entity")]
    Drops,
}

impl Related<super::drops::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Drops.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
