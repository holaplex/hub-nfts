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
                    .table(MetadataJsons::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MetadataJsons::CollectionId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MetadataJsons::Identifier)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MetadataJsons::Name).string().not_null())
                    .col(ColumnDef::new(MetadataJsons::Symbol).string().not_null())
                    .col(
                        ColumnDef::new(MetadataJsons::Description)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MetadataJsons::Image).string().not_null())
                    .col(ColumnDef::new(MetadataJsons::AnimationUrl).string())
                    .col(ColumnDef::new(MetadataJsons::ExternalUrl).string())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-metadata_jsons-collection_id")
                            .from(MetadataJsons::Table, MetadataJsons::CollectionId)
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
                    .name("metadata_jsons_identifier_idx")
                    .table(MetadataJsons::Table)
                    .col(MetadataJsons::Identifier)
                    .unique()
                    .index_type(IndexType::BTree)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("metadata_jsons_name_idx")
                    .table(MetadataJsons::Table)
                    .col(MetadataJsons::Name)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MetadataJsons::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum MetadataJsons {
    Table,
    CollectionId,
    Identifier,
    Name,
    Symbol,
    Description,
    Image,
    AnimationUrl,
    ExternalUrl,
}
