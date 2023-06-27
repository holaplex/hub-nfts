//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.0

use async_graphql::SimpleObject;
use sea_orm::entity::prelude::*;

use crate::proto;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, SimpleObject)]
#[sea_orm(table_name = "collection_creators")]
#[graphql(concrete(name = "CollectionCreator", params()))]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub collection_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub address: String,
    pub verified: bool,
    pub share: i32,
}

impl From<Model> for proto::Creator {
    fn from(
        Model {
            address,
            verified,
            share,
            ..
        }: Model,
    ) -> Self {
        Self {
            address,
            verified,
            share,
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
}

impl Related<super::collections::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Collections.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
