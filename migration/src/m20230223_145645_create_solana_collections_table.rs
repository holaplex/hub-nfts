use sea_orm_migration::prelude::*;

use crate::m20230214_212301_create_collections_table::Collections;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SolanaCollections::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SolanaCollections::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("default gen_random_uuid()".to_string()),
                    )
                    .col(
                        ColumnDef::new(SolanaCollections::CollectionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SolanaCollections::MasterEditionAddress)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SolanaCollections::UpdateAuthority)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SolanaCollections::AtaPubkey)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SolanaCollections::OwnerPubkey)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SolanaCollections::MintPubkey)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SolanaCollections::MetadataPubkey)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SolanaCollections::SellerFeeBasisPoints)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SolanaCollections::CreatedBy)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SolanaCollections::CreatedAt)
                            .timestamp()
                            .not_null()
                            .extra("default now()".to_string()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-drops_solana_collections_id")
                            .from(SolanaCollections::Table, SolanaCollections::CollectionId)
                            .to(Collections::Table, Collections::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("solana-collections_project_id_idx")
                    .table(SolanaCollections::Table)
                    .col(SolanaCollections::CollectionId)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("solana-collections_address_idx")
                    .table(SolanaCollections::Table)
                    .col(SolanaCollections::MasterEditionAddress)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SolanaCollections::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum SolanaCollections {
    Table,
    Id,
    CollectionId,
    MasterEditionAddress,
    SellerFeeBasisPoints,
    AtaPubkey,
    UpdateAuthority,
    OwnerPubkey,
    MintPubkey,
    MetadataPubkey,
    CreatedBy,
    CreatedAt,
}
