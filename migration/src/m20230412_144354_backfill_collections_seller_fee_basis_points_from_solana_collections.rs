use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"UPDATE collections SET seller_fee_basis_points = solana_collections.seller_fee_basis_points FROM collections c INNER JOIN solana_collections ON c.id = solana_collections.collection_id;"#.to_string(),
        );

        db.execute(stmt).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"UPDATE collections SET seller_fee_basis_points = null;"#.to_string(),
        );

        db.execute(stmt).await?;

        Ok(())
    }
}
