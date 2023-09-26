use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(MetadataJsons::Table)
                    .modify_column(ColumnDef::new(MetadataJsons::Uri).null())
                    .modify_column(ColumnDef::new(MetadataJsons::Identifier).null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(MetadataJsons::Table)
                    .modify_column(ColumnDef::new(MetadataJsons::Uri).not_null())
                    .modify_column(ColumnDef::new(MetadataJsons::Identifier).not_null())
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
    Identifier,
}
