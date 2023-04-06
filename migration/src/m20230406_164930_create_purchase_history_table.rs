use sea_orm_migration::prelude::*;

use crate::{
    m20230214_212301_create_collections_table::CreationStatus,
    m20230220_223223_create_collection_mints_table::CollectionMints,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Purchases::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Purchases::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("default gen_random_uuid()".to_string()),
                    )
                    .col(ColumnDef::new(Purchases::MintId).uuid().not_null())
                    .col(ColumnDef::new(Purchases::CustomerId).uuid().not_null())
                    .col(ColumnDef::new(Purchases::Wallet).text().not_null())
                    .col(ColumnDef::new(Purchases::Spent).big_integer().not_null())
                    .col(ColumnDef::new(Purchases::TxSignature).text())
                    .col(
                        ColumnDef::new(Purchases::Status)
                            .custom(CreationStatus::Type)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Purchases::CreatedAt)
                            .timestamp()
                            .not_null()
                            .extra("default now()".to_string()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-purchases_mint_id")
                            .from(Purchases::Table, Purchases::MintId)
                            .to(CollectionMints::Table, CollectionMints::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("purchases-mint_id_idx")
                    .table(Purchases::Table)
                    .col(Purchases::MintId)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("purchases-customer_id_idx")
                    .table(Purchases::Table)
                    .col(Purchases::CustomerId)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("purchases-wallet_idx")
                    .table(Purchases::Table)
                    .col(Purchases::Wallet)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("purchases-created-at_idx")
                    .table(Purchases::Table)
                    .col(Purchases::CreatedAt)
                    .index_type(IndexType::BTree)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Purchases::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Purchases {
    Table,
    Id,
    MintId,
    CustomerId,
    Wallet,
    TxSignature,
    Spent,
    CreatedAt,
    Status,
}
