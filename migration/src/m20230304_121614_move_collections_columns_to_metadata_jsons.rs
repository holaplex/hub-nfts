use sea_orm_migration::prelude::*;

use crate::m20230214_212301_create_collections_table::Collections;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Collections::Table)
                    .drop_column(Collections::Name)
                    .drop_column(Collections::Description)
                    .drop_column(Collections::MetadataUri)
                    .drop_column(Collections::RoyaltyWallet)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MetadataJsons::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(MetadataJsons::Uri).string().not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Collections::Table)
                    .add_column_if_not_exists(ColumnDef::new(Collections::Name).string().not_null())
                    .add_column_if_not_exists(
                        ColumnDef::new(Collections::Description).string().not_null(),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(Collections::MetadataUri).string().not_null(),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(Collections::RoyaltyWallet)
                            .string()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MetadataJsons::Table)
                    .drop_column(MetadataJsons::Uri)
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum MetadataJsons {
    Table,
    Uri,
}
