use sea_orm_migration::prelude::*;

use crate::m20230220_223223_create_collection_mints_table::CollectionMints;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(NftTransfers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(NftTransfers::TxSignature)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(NftTransfers::MintId).uuid().not_null())
                    .col(ColumnDef::new(NftTransfers::Sender).string().not_null())
                    .col(ColumnDef::new(NftTransfers::Receiver).string().not_null())
                    .col(
                        ColumnDef::new(NftTransfers::CreatedAt)
                            .timestamp()
                            .not_null()
                            .extra("default now()".to_string()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-nft_transfers_mint_id")
                            .from(NftTransfers::Table, NftTransfers::MintId)
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
                    .name("nft_transfers_mint_id_idx")
                    .table(NftTransfers::Table)
                    .col(NftTransfers::MintId)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("nft_transfers_sender_idx")
                    .table(NftTransfers::Table)
                    .col(NftTransfers::Sender)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("nft_transfers_receiver_idx")
                    .table(NftTransfers::Table)
                    .col(NftTransfers::Receiver)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("nft_transfers_created_at_idx")
                    .table(NftTransfers::Table)
                    .col(NftTransfers::CreatedAt)
                    .index_type(IndexType::BTree)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(NftTransfers::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum NftTransfers {
    Table,
    TxSignature,
    MintId,
    Sender,
    Receiver,
    CreatedAt,
}
