use sea_orm_migration::prelude::*;

use crate::m20230220_223223_create_collection_mints_table::CollectionMints;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("collection-mints-creation-status-idx")
                    .table(CollectionMints::Table)
                    .col(CollectionMints::CreationStatus)
                    .index_type(IndexType::BTree)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
