use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Drops::Table)
                    .add_column_if_not_exists(ColumnDef::new(Drops::PausedAt).timestamp())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Drops::Table)
                    .drop_column(Drops::PausedAt)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Drops {
    Table,
    PausedAt,
}
