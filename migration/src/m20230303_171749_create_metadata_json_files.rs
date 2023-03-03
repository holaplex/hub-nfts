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
                    .table(MetadataJsonFiles::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MetadataJsonFiles::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("default gen_random_uuid()".to_string()),
                    )
                    .col(
                        ColumnDef::new(MetadataJsonFiles::CollectionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MetadataJsonFiles::Uri).string())
                    .col(ColumnDef::new(MetadataJsonFiles::FileType).string())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-metadata_files-collection_id")
                            .from(MetadataJsonFiles::Table, MetadataJsonFiles::CollectionId)
                            .to(Collections::Table, Collections::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MetadataJsonFiles::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum MetadataJsonFiles {
    Table,
    Id,
    CollectionId,
    Uri,
    FileType,
}
