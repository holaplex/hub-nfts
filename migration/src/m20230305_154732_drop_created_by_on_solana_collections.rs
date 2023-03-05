use sea_orm_migration::prelude::*;

use super::m20230223_145645_create_solana_collections_table::SolanaCollections;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(SolanaCollections::Table)
                    .drop_column(SolanaCollections::CreatedBy)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(SolanaCollections::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(SolanaCollections::CreatedBy)
                            .uuid()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }
}
