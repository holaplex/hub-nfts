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
                    .table(CollectionCreators::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CollectionCreators::CollectionId)
                            .uuid()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(CollectionCreators::CollectionId)
                            .col(CollectionCreators::Address),
                    )
                    .col(
                        ColumnDef::new(CollectionCreators::Address)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CollectionCreators::Verified)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CollectionCreators::Share)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-creators-collection_id")
                            .from(CollectionCreators::Table, CollectionCreators::CollectionId)
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
                    .name("creators-address-idx")
                    .table(CollectionCreators::Table)
                    .col(CollectionCreators::Address)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CollectionCreators::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum CollectionCreators {
    Table,
    CollectionId,
    Address,
    Verified,
    Share,
}
