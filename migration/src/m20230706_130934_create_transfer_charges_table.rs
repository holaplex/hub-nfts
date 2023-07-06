use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TransferCharges::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TransferCharges::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("default gen_random_uuid()".to_string()),
                    )
                    .col(
                        ColumnDef::new(TransferCharges::CreditsDeductionId)
                            .uuid()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TransferCharges::Table).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum TransferCharges {
    Table,
    Id,
    CreditsDeductionId,
}
