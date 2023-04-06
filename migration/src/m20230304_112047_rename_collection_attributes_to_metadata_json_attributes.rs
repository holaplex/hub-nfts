use sea_orm_migration::prelude::*;

use crate::{
    m20230209_052038_create_collection_attributes_table::CollectionAttributes,
    m20230303_155836_add_metadata_json_tables::MetadataJsons,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk-collection_attributes-collection_id")
                    .table(CollectionAttributes::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .rename_table(
                Table::rename()
                    .table(CollectionAttributes::Table, MetadataJsonAttributes::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk-metadata_json_attributes-collection_id")
                    .from(
                        MetadataJsonAttributes::Table,
                        MetadataJsonAttributes::CollectionId,
                    )
                    .to(MetadataJsons::Table, MetadataJsons::CollectionId)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk-metadata_json_attributes-collection_id")
                    .table(MetadataJsonAttributes::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .rename_table(
                Table::rename()
                    .table(MetadataJsonAttributes::Table, CollectionAttributes::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk-collection_attributes-collection_id")
                    .from(
                        CollectionAttributes::Table,
                        CollectionAttributes::CollectionId,
                    )
                    .to(MetadataJsons::Table, MetadataJsons::CollectionId)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum MetadataJsonAttributes {
    Table,
    CollectionId,
}
