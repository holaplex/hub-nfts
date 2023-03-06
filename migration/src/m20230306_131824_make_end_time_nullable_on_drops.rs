use sea_orm_migration::prelude::*;

use super::m20230215_194724_create_drops_table::Drops;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Drops::Table)
                    .modify_column(ColumnDef::new(Drops::EndTime).null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Drops::Table)
                    .modify_column(ColumnDef::new(Drops::EndTime).not_null())
                    .to_owned(),
            )
            .await
    }
}
