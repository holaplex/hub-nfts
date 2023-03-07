//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.0

use async_graphql::SimpleObject;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, SimpleObject)]
#[sea_orm(table_name = "metadata_json_attributes")]
#[graphql(concrete(name = "MetadataJsonAttribute", params()))]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub collection_id: Uuid,
    #[sea_orm(column_type = "Text")]
    pub trait_type: String,
    #[sea_orm(column_type = "Text")]
    pub value: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::metadata_jsons::Entity",
        from = "Column::CollectionId",
        to = "super::metadata_jsons::Column::CollectionId",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    MetadataJsons,
}

impl Related<super::metadata_jsons::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MetadataJsons.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
