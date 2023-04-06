use sea_orm_migration::prelude::*;

use crate::{
    m20230214_212301_create_collections_table::Collections,
    m20230220_223223_create_collection_mints_table::CollectionMints,
    m20230303_155836_add_metadata_json_tables::MetadataJsons,
    m20230303_171749_create_metadata_json_files::MetadataJsonFiles,
    m20230304_112047_rename_collection_attributes_to_metadata_json_attributes::MetadataJsonAttributes,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(MetadataJsons::Table)
                    .drop_foreign_key(Alias::new("fk-metadata_jsons-collection_id"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MetadataJsons::Table)
                    .add_foreign_key(
                        TableForeignKey::new()
                            .name("fk_metadata_jsons_id-collections-id")
                            .from_tbl(MetadataJsons::Table)
                            .from_col(MetadataJsons::CollectionId)
                            .to_tbl(Collections::Table)
                            .to_col(Collections::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MetadataJsons::Table)
                    .add_foreign_key(
                        TableForeignKey::new()
                            .name("fk_metadata_jsons_id-collection_mints-id")
                            .from_tbl(MetadataJsons::Table)
                            .from_col(MetadataJsons::CollectionId)
                            .to_tbl(CollectionMints::Table)
                            .to_col(CollectionMints::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MetadataJsonAttributes::Table)
                    .drop_foreign_key(Alias::new("fk-metadata_json_attributes-collection_id"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MetadataJsonFiles::Table)
                    .drop_foreign_key(Alias::new("fk-metadata_files-collection_id"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MetadataJsonAttributes::Table)
                    .add_foreign_key(
                        TableForeignKey::new()
                            .name("fk_metadata_jsons_attributes_metadata_json_id")
                            .from_tbl(MetadataJsonAttributes::Table)
                            .from_col(MetadataJsonAttributes::CollectionId)
                            .to_tbl(MetadataJsons::Table)
                            .to_col(MetadataJsons::CollectionId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MetadataJsonFiles::Table)
                    .add_foreign_key(
                        TableForeignKey::new()
                            .name("fk_metadata_jsons_files_metadata_json_id")
                            .from_tbl(MetadataJsonFiles::Table)
                            .from_col(MetadataJsonFiles::CollectionId)
                            .to_tbl(MetadataJsons::Table)
                            .to_col(MetadataJsons::CollectionId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MetadataJsons::Table)
                    .rename_column(Alias::new("collection_id"), Alias::new("id"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MetadataJsonAttributes::Table)
                    .rename_column(Alias::new("collection_id"), Alias::new("metadata_json_id"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MetadataJsonFiles::Table)
                    .rename_column(Alias::new("collection_id"), Alias::new("metadata_json_id"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
