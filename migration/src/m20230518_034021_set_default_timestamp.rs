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
            r#"alter table drops alter column start_time set default now();"#.to_string(),
        );

        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"alter table drops alter column created_at set default now();"#.to_string(),
        );

        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"alter table nft_transfers alter column created_at set default now();"#.to_string(),
        );

        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"alter table purchases alter column created_at set default now();"#.to_string(),
        );

        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"alter table solana_collections alter column created_at set default now();"#
                .to_string(),
        );

        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"alter table collection_mints alter column created_at set default now();"#
                .to_string(),
        );

        db.execute(stmt).await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
