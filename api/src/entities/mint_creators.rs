//! `SeaORM` Entity. Generated by sea-orm-codegen 0.10.5

use async_graphql::SimpleObject;
use sea_orm::{entity::prelude::*, Set};

use crate::proto;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, SimpleObject)]
#[sea_orm(table_name = "mint_creators")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub collection_mint_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub address: String,
    pub verified: bool,
    pub share: i32,
}

impl ActiveModelBehavior for ActiveModel {
    /// Will be triggered before insert / update
    fn before_save(mut self, _insert: bool) -> Result<Self, DbErr> {
        if self.address.as_ref().starts_with("0x") {
            self.address = Set(self.address.as_ref().to_lowercase());
        }

        Ok(self)
    }
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
        belongs_to = "super::collection_mints::Entity",
        from = "Column::CollectionMintId",
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

impl Entity {
    pub fn find_by_collection_mint_id(collection_mint_id: Uuid) -> Select<Self> {
        Self::find().filter(Column::CollectionMintId.eq(collection_mint_id))
    }
}
