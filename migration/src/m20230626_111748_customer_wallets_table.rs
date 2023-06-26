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
                    .table(CustomerWallets::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CustomerWallets::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CustomerWallets::CustomerId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CustomerWallets::Address).text().not_null())
                    .col(
                        ColumnDef::new(CustomerWallets::Blockchain)
                            .custom(Blockchain::Type)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CustomerWallets::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum CustomerWallets {
    Table,
    Id,
    CustomerId,
    Address,
    Blockchain,
}
