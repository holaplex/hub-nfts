use sea_orm_migration::prelude::*;

use crate::m20230220_223223_create_collection_mints_table::CollectionMints;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MintCreators::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MintCreators::CollectionMintId)
                            .uuid()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(MintCreators::CollectionMintId)
                            .col(MintCreators::Address),
                    )
                    .col(ColumnDef::new(MintCreators::Address).string().not_null())
                    .col(ColumnDef::new(MintCreators::Verified).boolean().not_null())
                    .col(ColumnDef::new(MintCreators::Share).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-mints_creators-collection_mint_id")
                            .from(MintCreators::Table, MintCreators::CollectionMintId)
                            .to(CollectionMints::Table, CollectionMints::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("mint_creators-address-idx")
                    .table(MintCreators::Table)
                    .col(MintCreators::Address)
                    .index_type(IndexType::Hash)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MintCreators::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum MintCreators {
    Table,
    CollectionMintId,
    Address,
    Verified,
    Share,
}
