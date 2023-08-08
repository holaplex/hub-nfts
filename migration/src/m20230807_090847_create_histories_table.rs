use sea_orm_migration::prelude::*;

use crate::{
    m20230214_212301_create_collections_table::CreationStatus,
    m20230220_223223_create_collection_mints_table::CollectionMints,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UpdateHistories::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UpdateHistories::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("default gen_random_uuid()".to_string()),
                    )
                    .col(ColumnDef::new(UpdateHistories::MintId).uuid().not_null())
                    .col(ColumnDef::new(UpdateHistories::TxnSignature).text())
                    .col(
                        ColumnDef::new(UpdateHistories::Status)
                            .custom(CreationStatus::Type)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UpdateHistories::CreditDeductionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(UpdateHistories::CreatedBy).uuid().not_null())
                    .col(
                        ColumnDef::new(UpdateHistories::CreatedAt)
                            .timestamp()
                            .not_null()
                            .extra("default now()".to_string()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-update_histories_mint_id")
                            .from(UpdateHistories::Table, UpdateHistories::MintId)
                            .to(CollectionMints::Table, CollectionMints::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UpdateHistories::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum UpdateHistories {
    Table,
    Id,
    MintId,
    TxnSignature,
    Status,
    CreditDeductionId,
    CreatedBy,
    CreatedAt,
}
