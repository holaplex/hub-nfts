use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Collections::Table)
                    .add_column_if_not_exists(ColumnDef::new(Collections::CreatedBy).uuid())
                    .add_column_if_not_exists(
                        ColumnDef::new(Collections::CreatedAt)
                            .timestamp_with_time_zone()
                            .extra("default now()".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        let db = manager.get_connection();

        let stmt = Statement::from_string(
                manager.get_database_backend(),
                r#"UPDATE collections SET created_by = drops.created_by, created_at = drops.created_at FROM drops WHERE drops.collection_id = collections.id;"#.to_string(),
            );

        db.execute(stmt).await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Collections::Table)
                    .modify_column(ColumnDef::new(Collections::CreatedBy).not_null())
                    .modify_column(ColumnDef::new(Collections::CreatedAt).not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Collections::Table)
                    .drop_column(Collections::CreatedBy)
                    .drop_column(Collections::CreatedAt)
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Collections {
    Table,
    CreatedBy,
    CreatedAt,
}
