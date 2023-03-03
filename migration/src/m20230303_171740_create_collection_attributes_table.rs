use sea_orm_migration::prelude::*;

use crate::{
    m20230209_052038_create_collection_attributes_table::CollectionAttributes,
    m20230214_212301_create_collections_table::Collections,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk-collection_attributes-collection_id")
                    .from(
                        CollectionAttributes::Table,
                        CollectionAttributes::CollectionId,
                    )
                    .to(Collections::Table, Collections::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("collection_attributes_unique_index")
                    .table(CollectionAttributes::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("collection_attributes_collection_id")
                    .table(CollectionAttributes::Table)
                    .col(CollectionAttributes::CollectionId)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk-collection_attributes-collection_id")
                    .table(CollectionAttributes::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("collection_attributes_unique_index")
                    .table(CollectionAttributes::Table)
                    .col(CollectionAttributes::TraitType)
                    .col(CollectionAttributes::Value)
                    .unique()
                    .index_type(IndexType::BTree)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("collection_attributes_collection_id")
                    .table(CollectionAttributes::Table)
                    .to_owned(),
            )
            .await
    }
}
