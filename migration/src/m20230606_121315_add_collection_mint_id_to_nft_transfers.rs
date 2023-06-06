use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::m20230220_223223_create_collection_mints_table::CollectionMints;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        manager
            .alter_table(
                Table::alter()
                    .table(NftTransfers::Table)
                    .add_column(ColumnDef::new(NftTransfers::CollectionMintId).uuid().null())
                    .to_owned(),
            )
            .await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"UPDATE nft_transfers SET collection_mint_id = collection_mints.id FROM nft_transfers nt INNER JOIN collection_mints ON nt.mint_address = collection_mints.address;"#.to_string(),
        );

        db.execute(stmt).await?;

        manager
            .alter_table(
                Table::alter()
                    .table(NftTransfers::Table)
                    .drop_column(NftTransfers::MintAddress)
                    .modify_column(ColumnDef::new(NftTransfers::CollectionMintId).not_null())
                    .add_foreign_key(
                        TableForeignKey::new()
                            .name("fk-nft_transfers_collection_mint_id")
                            .from_tbl(NftTransfers::Table)
                            .from_col(NftTransfers::CollectionMintId)
                            .to_tbl(CollectionMints::Table)
                            .to_col(CollectionMints::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        manager
            .alter_table(
                Table::alter()
                    .table(NftTransfers::Table)
                    .add_column(ColumnDef::new(NftTransfers::MintAddress).string().null())
                    .to_owned(),
            )
            .await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"UPDATE nft_transfers SET mint_address = collection_mints.address FROM nft_transfers nt INNER JOIN collection_mints ON nt.collection_mint_id = collection_mints.id;"#.to_string(),
        );

        db.execute(stmt).await?;

        manager
            .alter_table(
                Table::alter()
                    .table(NftTransfers::Table)
                    .drop_column(NftTransfers::CollectionMintId)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(NftTransfers::Table)
                    .modify_column(ColumnDef::new(NftTransfers::MintAddress).not_null())
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum NftTransfers {
    Table,
    CollectionMintId,
    MintAddress,
}
