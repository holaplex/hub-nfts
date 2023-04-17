use sea_orm_migration::prelude::*;

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
                        ColumnDef::new(NftTransfers::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("default gen_random_uuid()".to_string()),
                    )
                    .col(ColumnDef::new(NftTransfers::TxSignature).string())
                    .col(
                        ColumnDef::new(NftTransfers::MintAddress)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(NftTransfers::Sender).string().not_null())
                    .col(ColumnDef::new(NftTransfers::Recipient).string().not_null())
                    .col(
                        ColumnDef::new(NftTransfers::CreatedAt)
                            .timestamp()
                            .not_null()
                            .extra("default now()".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("nft_transfers_mint_address_idx")
                    .table(NftTransfers::Table)
                    .col(NftTransfers::MintAddress)
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
                    .col(NftTransfers::Recipient)
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
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("nft_transfers_tx_signature_idx")
                    .table(NftTransfers::Table)
                    .col(NftTransfers::TxSignature)
                    .index_type(IndexType::Hash)
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
    Id,
    TxSignature,
    MintAddress,
    Sender,
    Recipient,
    CreatedAt,
}
