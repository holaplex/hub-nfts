use sea_orm_migration::{prelude::*, sea_query::extension::postgres::Type};

use crate::m20230214_212301_create_collections_table::CreationStatus;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_type(
                Type::alter()
                    .name(CreationStatus::Type)
                    .add_value(Alias::new("queued"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
