use sea_orm_migration::prelude::*;

use crate::{
    m20230214_212301_create_collections_table::Collections,
    m20230220_223223_create_collection_mints_table::CollectionMints,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Collections::Table)
                    .add_column_if_not_exists(ColumnDef::new(Collections::Signature).string())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CollectionMints::Table)
                    .add_column_if_not_exists(ColumnDef::new(CollectionMints::Signature).string())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Collections::Table)
                    .drop_column(Collections::Signature)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CollectionMints::Table)
                    .drop_column(CollectionMints::Signature)
                    .to_owned(),
            )
            .await
    }
}
