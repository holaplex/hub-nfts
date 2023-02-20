//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.0

use sea_orm::entity::prelude::*;

use super::sea_orm_active_enums::CreationStatus;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "drops")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub project_id: Uuid,
    pub organization_id: Uuid,
    pub collection_id: Uuid,
    pub creation_status: CreationStatus,
    pub start_time: DateTime,
    pub end_time: DateTime,
    pub price: i64,
    pub created_by: Uuid,
    pub created_at: DateTime,
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
