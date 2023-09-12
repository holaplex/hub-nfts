use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, Statement};

use crate::m20230303_155836_add_metadata_json_tables::MetadataJsons;
use crate::m20230304_121614_move_collections_columns_to_metadata_jsons::MetadataJsons as MetadataJsons2;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MetadataJsonUploads::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MetadataJsonUploads::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MetadataJsonUploads::Uri).string().not_null())
                    .col(
                        ColumnDef::new(MetadataJsonUploads::Identifier)
                            .string()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-metadata_json_uploads_metadata_json_id")
                            .from(MetadataJsonUploads::Table, MetadataJsonUploads::Id)
                            .to(MetadataJsons::Table, Alias::new("id")), // ???
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "insert into metadata_json_uploads (id, uri, identifier) \
                    select id, uri, identifier from metadata_jsons"
                    .into(),
            ))
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MetadataJsons::Table)
                    .drop_column(MetadataJsons::Identifier)
                    .drop_column(MetadataJsons2::Uri)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(MetadataJsons::Table)
                    .add_column_if_not_exists(ColumnDef::new(MetadataJsons2::Uri).string().null())
                    .add_column_if_not_exists(
                        ColumnDef::new(MetadataJsons::Identifier).string().null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "update metadata_jsons \
                    set uri = rows.uri, \
                        identifier = rows.identifier \
                    from (select id, uri, identifier from metadata_json_uploads) as rows \
                    where rows.id = metadata_jsons.id"
                    .into(),
            ))
            .await?;

        manager
            .drop_table(Table::drop().table(MetadataJsonUploads::Table).to_owned())
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MetadataJsons::Table)
                    .modify_column(ColumnDef::new(MetadataJsons2::Uri).not_null())
                    .modify_column(ColumnDef::new(MetadataJsons::Identifier).not_null())
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum MetadataJsonUploads {
    Table,
    Id,
    Uri,
    Identifier,
}
