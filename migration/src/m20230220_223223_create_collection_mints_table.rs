use sea_orm_migration::prelude::*;

use crate::m20230214_212301_create_collections_table::{Collections, CreationStatus};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CollectionMints::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CollectionMints::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("default gen_random_uuid()".to_string()),
                    )
                    .col(
                        ColumnDef::new(CollectionMints::CollectionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CollectionMints::Address).text().not_null())
                    .col(ColumnDef::new(CollectionMints::Owner).text().not_null())
                    .col(
                        ColumnDef::new(CollectionMints::CreationStatus)
                            .custom(CreationStatus::Type)
                            .not_null(),
                    )
                    .col(ColumnDef::new(CollectionMints::CreatedBy).uuid().not_null())
                    .col(
                        ColumnDef::new(CollectionMints::CreatedAt)
                            .timestamp()
                            .not_null()
                            .extra("default now()".to_string()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-drops_collections_id")
                            .from(CollectionMints::Table, CollectionMints::CollectionId)
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
                    .name("collection-mints_collection_id_idx")
                    .table(CollectionMints::Table)
                    .col(CollectionMints::CollectionId)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("collection-mints_address_idx")
                    .table(CollectionMints::Table)
                    .col(CollectionMints::Address)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("collection-mints_owner_idx")
                    .table(CollectionMints::Table)
                    .col(CollectionMints::Owner)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CollectionMints::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum CollectionMints {
    Table,
    Id,
    CollectionId,
    Address,
    Owner,
    CreationStatus,
    CreatedBy,
    CreatedAt,
}
