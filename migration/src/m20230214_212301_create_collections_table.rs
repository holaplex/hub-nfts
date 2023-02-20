use sea_orm_migration::{prelude::*, sea_query::extension::postgres::Type};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type(
                Type::create()
                    .as_enum(Blockchain::Type)
                    .values([Blockchain::Solana, Blockchain::Polygon])
                    .to_owned(),
            )
            .await?;

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
                    .table(Collections::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Collections::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("default gen_random_uuid()".to_string()),
                    )
                    .col(ColumnDef::new(Collections::Collection).uuid().not_null())
                    .col(
                        ColumnDef::new(Collections::Blockchain)
                            .custom(Blockchain::Type)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Collections::Name).string().not_null())
                    .col(ColumnDef::new(Collections::Description).string().not_null())
                    .col(ColumnDef::new(Collections::MetadataUri).string().not_null())
                    .col(
                        ColumnDef::new(Collections::RoyaltyWallet)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Collections::Supply).big_integer())
                    .col(
                        ColumnDef::new(Collections::CreationStatus)
                            .custom(CreationStatus::Type)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("solana-collections_name_idx")
                    .table(Collections::Table)
                    .col(Collections::Name)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Collections::Table).to_owned())
            .await?;

        manager
            .drop_type(
                Type::drop()
                    .if_exists()
                    .name(CreationStatus::Type)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_type(Type::drop().if_exists().name(Blockchain::Type).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Collections {
    Table,
    Id,
    Collection,
    Blockchain,
    Name,
    Description,
    MetadataUri,
    RoyaltyWallet,
    Supply,
    CreationStatus,
}

pub enum Blockchain {
    Type,
    Solana,
    Polygon,
}

impl Iden for Blockchain {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(s, "{}", match self {
            Self::Type => "blockchain",
            Self::Solana => "solana",
            Self::Polygon => "polyogn",
        })
        .unwrap();
    }
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
