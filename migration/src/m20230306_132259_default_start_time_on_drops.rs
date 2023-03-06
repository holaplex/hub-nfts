use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

use crate::sea_orm::Statement;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"ALTER TABLE drops ALTER COLUMN start_time SET DEFAULT now();"#.to_string(),
        );

        db.execute(stmt).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"ALTER TABLE drops ALTER COLUMN start_time DROP DEFAULT;"#.to_string(),
        );

        db.execute(stmt).await?;

        Ok(())
    }
}
