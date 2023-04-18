use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(SolanaCollections::Table)
                    .drop_column(SolanaCollections::SellerFeeBasisPoints)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(SolanaCollections::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(SolanaCollections::SellerFeeBasisPoints).small_integer(),
                    )
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum SolanaCollections {
    Table,
    SellerFeeBasisPoints,
}
