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
                    .col(ColumnDef::new(Drops::CollectionId).uuid().not_null())
                    .col(
                        ColumnDef::new(Drops::CreationStatus)
                            .custom(CreationStatus::Type)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Drops::StartTime).timestamp().not_null())
                    .col(ColumnDef::new(Drops::EndTime).timestamp().not_null())
                    .col(ColumnDef::new(Drops::Price).big_integer().not_null())
                    .col(ColumnDef::new(Drops::CreatedBy).uuid().not_null())
                    .col(
                        ColumnDef::new(Drops::CreatedAt)
                            .timestamp()
                            .not_null()
                            .extra("default now()".to_string()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-drops_collections_id")
                            .from(Drops::Table, Drops::CollectionId)
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
    CollectionId,
    CreationStatus,
    StartTime,
    EndTime,
    Price,
    CreatedBy,
    CreatedAt,
}
