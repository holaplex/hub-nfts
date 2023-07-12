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
                    .add_column_if_not_exists(
                        ColumnDef::new(Collections::CreditsDeductionId).uuid(),
                    )
                    .add_column_if_not_exists(ColumnDef::new(Collections::ProjectId).uuid())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("collections-project_id_index")
                    .table(Collections::Table)
                    .col(Collections::ProjectId)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("collections-credits_deduction_id_index")
                    .table(Collections::Table)
                    .col(Collections::CreditsDeductionId)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await?;

        let db = manager.get_connection();

        let stmt = Statement::from_string(
                manager.get_database_backend(),
                r#"UPDATE collections SET credits_deduction_id = drops.credits_deduction_id, project_id = drops.project_id FROM collections c INNER JOIN drops ON c.id = drops.collection_id;"#.to_string(),
            );

        db.execute(stmt).await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Collections::Table)
                    .modify_column(ColumnDef::new(Collections::ProjectId).not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Collections::Table)
                    .drop_column(Collections::CreditsDeductionId)
                    .drop_column(Collections::ProjectId)
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Collections {
    Table,
    CreditsDeductionId,
    ProjectId,
}
