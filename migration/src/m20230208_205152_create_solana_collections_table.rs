use sea_orm_migration::prelude::*;

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
                        ColumnDef::new(SolanaCollections::ProjectId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SolanaCollections::Address)
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
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("solana-collections_project_id_idx")
                    .table(SolanaCollections::Table)
                    .col(SolanaCollections::ProjectId)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("solana-collections_address_idx")
                    .table(SolanaCollections::Table)
                    .col(SolanaCollections::Address)
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
    ProjectId,
    Address,
    SellerFeeBasisPoints,
    AtaPubkey,
    UpdateAuthority,
    OwnerPubkey,
    MintPubkey,
    MetadataPubkey,
    CreatedBy,
    CreatedAt,
}
