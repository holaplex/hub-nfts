//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.2

use super::sea_orm_active_enums::MetadataJsonJobType;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "metadata_json_jobs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub r#type: MetadataJsonJobType,
    pub continuation: Option<Vec<u8>>,
    pub failed: bool,
    pub url: Option<String>,
    pub metadata_json_id: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::metadata_jsons::Entity",
        from = "Column::MetadataJsonId",
        to = "super::metadata_jsons::Column::Id",
        on_update = "Cascade",
        on_delete = "NoAction"
    )]
    MetadataJsons,
}

impl Related<super::metadata_jsons::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MetadataJsons.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
