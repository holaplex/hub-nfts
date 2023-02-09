use sea_orm_migration::prelude::*;

use crate::m20230208_205152_create_solana_collections_table::CreationStatus;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Drops::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Drops::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("default gen_random_uuid()".to_string()),
                    )
                    .col(ColumnDef::new(Drops::ProjectId).uuid().not_null())
                    .col(ColumnDef::new(Drops::OrganizationId).uuid().not_null())
                    .col(ColumnDef::new(Drops::CollectionId).uuid().not_null())
                    .col(
                        ColumnDef::new(Drops::CreationStatus)
                            .custom(CreationStatus::Type)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Drops::StartTime).timestamp())
                    .col(ColumnDef::new(Drops::EndTime).timestamp())
                    .col(ColumnDef::new(Drops::Price).big_integer())
                    .col(ColumnDef::new(Drops::CreatedBy).uuid())
                    .col(
                        ColumnDef::new(Drops::CreatedAt)
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
                    .name("drops_project_id_idx")
                    .table(Drops::Table)
                    .col(Drops::ProjectId)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("drops_organization_id_idx")
                    .table(Drops::Table)
                    .col(Drops::OrganizationId)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("drops_collection_id_idx")
                    .table(Drops::Table)
                    .col(Drops::CollectionId)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Drops::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Drops {
    Table,
    Id,
    ProjectId,
    OrganizationId,
    CollectionId,
    CreationStatus,
    StartTime,
    EndTime,
    Price,
    CreatedBy,
    CreatedAt,
}
