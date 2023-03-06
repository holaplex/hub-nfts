use sea_orm_migration::prelude::*;

use super::m20230303_155836_add_metadata_json_tables::MetadataJsons;
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                IndexDropStatement::new()
                    .name("metadata_jsons_identifier_idx")
                    .table(MetadataJsons::Table)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
            .await
    }
}
