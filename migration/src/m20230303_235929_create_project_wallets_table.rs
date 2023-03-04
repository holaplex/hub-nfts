use sea_orm_migration::prelude::*;

use crate::m20230214_212301_create_collections_table::Blockchain;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProjectWallets::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProjectWallets::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("default gen_random_uuid()".to_string()),
                    )
                    .col(ColumnDef::new(ProjectWallets::ProjectId).uuid().not_null())
                    .col(
                        ColumnDef::new(ProjectWallets::WalletAddress)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProjectWallets::Blockchain)
                            .custom(Blockchain::Type)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("project_wallets-project-id-idx")
                    .table(ProjectWallets::Table)
                    .col(ProjectWallets::ProjectId)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("project_wallet_address-idx")
                    .table(ProjectWallets::Table)
                    .col(ProjectWallets::WalletAddress)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ProjectWallets::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum ProjectWallets {
    Table,
    Id,
    ProjectId,
    WalletAddress,
    Blockchain,
}
