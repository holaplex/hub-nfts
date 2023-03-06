//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.0

use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use sea_orm::entity::prelude::*;

use super::{
    collection_mints, metadata_jsons,
    sea_orm_active_enums::{Blockchain, CreationStatus},
};
use crate::AppContext;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "collections")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub blockchain: Blockchain,
    pub supply: Option<i64>,
    pub creation_status: CreationStatus,
    #[sea_orm(column_type = "Text")]
    pub address: Option<String>,
    pub total_mints: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, SimpleObject)]
#[graphql(complex)]
pub struct Collection {
    pub id: Uuid,
    pub blockchain: Blockchain,
    pub supply: Option<i64>,
    pub creation_status: CreationStatus,
    pub address: Option<String>,
    pub total_mints: i64,
}

#[ComplexObject]
impl Collection {
    async fn metadata_json(&self, ctx: &Context<'_>) -> Result<Option<metadata_jsons::Model>> {
        let AppContext {
            metadata_json_loader,
            ..
        } = ctx.data::<AppContext>()?;

        metadata_json_loader.load_one(self.id).await
    }

    async fn mints(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<Vec<collection_mints::CollectionMint>>> {
        let AppContext {
            collection_mints_loader,
            ..
        } = ctx.data::<AppContext>()?;

        collection_mints_loader.load_one(self.id).await
    }
}

impl From<Model> for Collection {
    fn from(
        Model {
            id,
            blockchain,
            supply,
            creation_status,
            address,
            total_mints,
        }: Model,
    ) -> Self {
        Self {
            id,
            blockchain,
            supply,
            creation_status,
            address,
            total_mints,
        }
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_one = "super::drops::Entity")]
    Drops,
}

impl Related<super::drops::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Drops.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
