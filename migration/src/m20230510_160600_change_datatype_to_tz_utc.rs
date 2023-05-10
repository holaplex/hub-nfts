use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, Statement},
};

use crate::{
    m20230215_194724_create_drops_table::Drops,
    m20230220_223223_create_collection_mints_table::CollectionMints,
    m20230223_145645_create_solana_collections_table::SolanaCollections,
    m20230406_164930_create_purchase_history_table::Purchases,
    m20230411_230029_create_nft_transfers_table::NftTransfers,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"alter database nfts set timezone to 'utc' ;"#.to_string(),
        );

        db.execute(stmt).await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Drops::Table)
                    .modify_column(
                        ColumnDef::new(Alias::new("start_time"))
                            .timestamp_with_time_zone()
                            .default("now()"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Drops::Table)
                    .modify_column(
                        ColumnDef::new(Alias::new("paused_at")).timestamp_with_time_zone(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Drops::Table)
                    .modify_column(
                        ColumnDef::new(Alias::new("shutdown_at")).timestamp_with_time_zone(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Drops::Table)
                    .modify_column(
                        ColumnDef::new(Alias::new("end_time")).timestamp_with_time_zone(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Drops::Table)
                    .modify_column(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null()
                            .default("now()"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(NftTransfers::Table)
                    .modify_column(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null()
                            .default("now()"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Purchases::Table)
                    .modify_column(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null()
                            .default("now()"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(SolanaCollections::Table)
                    .modify_column(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null()
                            .default("now()"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CollectionMints::Table)
                    .modify_column(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null()
                            .default("now()"),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
