use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // manager
        //     .alter_table(
        //         Table::alter()
        //             .table(CollectionCreators::Table)
        //             .rename_column(
        //                 CollectionCreators::CollectionId,
        //                 CollectionCreators::ParentId,
        //             )
        //             .to_owned(),
        //     )
        //     .await?;

        // manager
        //     .rename_table(
        //         Table::rename()
        //             .table(CollectionCreators::Table, Creators::Table)
        //             .to_owned(),
        //     )
        //     .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Creators::Table)
                    .add_foreign_key(
                        TableForeignKey::new()
                            .name("fk-creators-collection_mint-parent_id")
                            .from_tbl(Creators::Table)
                            .from_col(Creators::ParentId)
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
        manager
            .rename_table(
                Table::rename()
                    .table(Creators::Table, CollectionCreators::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CollectionCreators::Table)
                    .rename_column(
                        CollectionCreators::ParentId,
                        CollectionCreators::CollectionId,
                    )
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum CollectionMints {
    Table,
    Id,
}

#[derive(Iden)]
enum CollectionCreators {
    Table,
    CollectionId,
    ParentId,
}

#[derive(Iden)]
enum Creators {
    Table,
    ParentId,
}
