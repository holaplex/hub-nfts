//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.0

use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use sea_orm::entity::prelude::*;

use super::{collections, sea_orm_active_enums::CreationStatus};
use crate::AppContext;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, SimpleObject)]
#[sea_orm(table_name = "drops")]
#[graphql(complex, concrete(name = "Drop", params()))]

pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub project_id: Uuid,
    pub collection_id: Uuid,
    pub creation_status: CreationStatus,
    pub start_time: Option<DateTime>,
    pub end_time: Option<DateTime>,
    pub price: i64,
    pub created_by: Uuid,
    pub created_at: DateTime,
}

#[ComplexObject]
impl Model {
    async fn collection(&self, ctx: &Context<'_>) -> Result<Option<collections::Collection>> {
        let AppContext {
            collection_loader, ..
        } = ctx.data::<AppContext>()?;

        collection_loader.load_one(self.collection_id).await
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
