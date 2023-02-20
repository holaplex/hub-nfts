use sea_orm_migration::{prelude::*, sea_query::extension::postgres::Type};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type(
                Type::create()
                    .as_enum(CreationStatus::Type)
                    .values([CreationStatus::Pending, CreationStatus::Created])
                    .to_owned(),
            )
            .await?;

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
                    .col(ColumnDef::new(SolanaCollections::Name).string().not_null())
                    .col(
                        ColumnDef::new(SolanaCollections::Description)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SolanaCollections::MetadataUri)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SolanaCollections::AnimationUri).string())
                    .col(
                        ColumnDef::new(SolanaCollections::ImageUri)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SolanaCollections::ExternalLink).string())
                    .col(
                        ColumnDef::new(SolanaCollections::SellerFeeBasisPoints)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SolanaCollections::RoyaltyWallet)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SolanaCollections::Supply).big_integer())
                    .col(
                        ColumnDef::new(SolanaCollections::CreationStatus)
                            .custom(CreationStatus::Type)
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
                    .name("solana-collections_organization_id_idx")
                    .table(SolanaCollections::Table)
                    .col(SolanaCollections::OrganizationId)
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
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("solana-collections_name_idx")
                    .table(SolanaCollections::Table)
                    .col(SolanaCollections::Name)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SolanaCollections::Table).to_owned())
            .await?;

        manager
            .drop_type(
                Type::drop()
                    .if_exists()
                    .name(CreationStatus::Type)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum SolanaCollections {
    Table,
    Id,
    ProjectId,
    OrganizationId,
    Address,
    Name,
    Description,
    MetadataUri,
    AnimationUri,
    ImageUri,
    ExternalLink,
    SellerFeeBasisPoints,
    RoyaltyWallet,
    Supply,
    CreationStatus,
    AtaPubkey,
    UpdateAuthority,
    OwnerPubkey,
    MintPubkey,
    MetadataPubkey,
    CreatedBy,
    CreatedAt,
}

pub enum CreationStatus {
    Type,
    Pending,
    Created,
}

impl Iden for CreationStatus {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(s, "{}", match self {
            Self::Type => "creation_status",
            Self::Pending => "pending",
            Self::Created => "created",
        })
        .unwrap();
    }
}
