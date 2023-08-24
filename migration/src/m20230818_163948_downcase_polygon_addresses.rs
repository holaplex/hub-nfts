use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, Statement},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"UPDATE collection_creators
            SET address = LOWER(address)
            WHERE SUBSTRING(address, 1, 2) = '0x';"#
                .to_string(),
        );

        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"UPDATE collection_mints
            SET address = LOWER(address), owner=LOWER(owner)
            WHERE SUBSTRING(owner, 1, 2) = '0x';"#
                .to_string(),
        );

        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"UPDATE collections
            SET address = LOWER(address)
            WHERE SUBSTRING(address, 1, 2) = '0x';"#
                .to_string(),
        );

        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"UPDATE customer_wallets
            SET address = LOWER(address)
            WHERE SUBSTRING(address, 1, 2) = '0x';"#
                .to_string(),
        );

        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"UPDATE mint_creators
            SET address = LOWER(address)
            WHERE SUBSTRING(address, 1, 2) = '0x';"#
                .to_string(),
        );

        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"UPDATE mint_histories
            SET wallet = LOWER(wallet)
            WHERE SUBSTRING(wallet, 1, 2) = '0x';"#
                .to_string(),
        );

        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"UPDATE nft_transfers
            SET sender = LOWER(sender), recipient = LOWER(recipient)
            WHERE SUBSTRING(sender, 1, 2) = '0x';"#
                .to_string(),
        );

        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"UPDATE project_wallets
            SET wallet_address = LOWER(wallet_address)
            WHERE SUBSTRING(wallet_address, 1, 2) = '0x';"#
                .to_string(),
        );

        db.execute(stmt).await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
