use sea_orm::entity::prelude::*;
use serde_json::Value as Json;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "job_trackings")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub job_type: String,
    pub payload: Json,
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}