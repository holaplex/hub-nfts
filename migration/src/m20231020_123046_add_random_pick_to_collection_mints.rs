use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CollectionMints::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(CollectionMints::RandomPick)
                            .big_integer()
                            .not_null()
                            .default(Expr::cust(
                                "(floor(random() * 9223372036854775807))::bigint",
                            )),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("collection-mints-random_pick-idx")
                    .table(CollectionMints::Table)
                    .col((CollectionMints::RandomPick, IndexOrder::Asc))
                    .index_type(IndexType::BTree)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        manager
            .alter_table(
                Table::alter()
                    .table(CollectionMints::Table)
                    .drop_column(CollectionMints::RandomPick)
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum CollectionMints {
    Table,
    RandomPick,
}
