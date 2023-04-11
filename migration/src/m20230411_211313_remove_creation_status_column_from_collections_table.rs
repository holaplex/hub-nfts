use sea_orm_migration::prelude::*;

use crate::m20230214_212301_create_collections_table::{Collections, CreationStatus};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Collections::Table)
                    .drop_column(Collections::CreationStatus)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Collections::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Collections::CreationStatus)
                            .custom(CreationStatus::Type)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }
}
