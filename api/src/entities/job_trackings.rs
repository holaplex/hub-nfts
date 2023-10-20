use hub_core::chrono;
use sea_orm::{entity::prelude::*, QueryOrder, Set};
use serde_json::Value as Json;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "job_trackings")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub job_type: String,
    pub payload: Json,
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    // Find a job tracking record by its ID
    pub fn find_by_id(id: i32) -> Select<Self> {
        Self::find().filter(Column::Id.eq(id))
    }

    // Create a new job tracking record
    pub fn create(job_type: &str, payload: Json, status: &str) -> ActiveModel {
        let now: DateTimeWithTimeZone = chrono::Utc::now().into();

        ActiveModel {
            job_type: Set(job_type.to_string()),
            payload: Set(payload),
            status: Set(status.to_string()),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
    }

    // Update the status of an existing job tracking record
    pub fn update_status(model: Model, new_status: &str) -> ActiveModel {
        let now: DateTimeWithTimeZone = chrono::Utc::now().into();
        let mut active_model: ActiveModel = model.into();

        active_model.status = Set(new_status.to_string());
        active_model.updated_at = Set(now);

        active_model
    }

    pub fn filter_failed_for_job_type(job_type: String) -> Select<Self> {
        Self::find()
            .filter(
                Column::Status
                    .eq("failed")
                    .and(Column::JobType.eq(job_type)),
            )
            .order_by_asc(Column::CreatedAt)
    }
}
