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
            r#"ALTER TABLE purchases
                RENAME TO mint_history;"#
                .to_string(),
        );

        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"ALTER TABLE mint_history
        ADD COLUMN collection UUID CONSTRAINT mint_history_collection_fk REFERENCES collections(id)
        ON UPDATE CASCADE ON DELETE CASCADE;"#
                .to_string(),
        );
        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#" UPDATE MINT_HISTORY SET COLLECTION = C.ID
                FROM DROPS D
                INNER JOIN COLLECTIONS C ON D.COLLECTION_ID = C.ID
                WHERE MINT_HISTORY.DROP_ID = D.ID AND MINT_HISTORY.DROP_ID IS NOT NULL;"#
                .to_string(),
        );
        db.execute(stmt).await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"ALTER TABLE mint_history
            DROP COLUMN drop_id;"#
                .to_string(),
        );

        db.execute(stmt).await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
