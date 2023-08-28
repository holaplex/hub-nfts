use sea_orm_migration::prelude::*;

use crate::m20230214_212301_create_collections_table::CreationStatus;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SwitchCollectionHistories::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SwitchCollectionHistories::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("default gen_random_uuid()".to_string()),
                    )
                    .col(
                        ColumnDef::new(SwitchCollectionHistories::CollectionMintId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SwitchCollectionHistories::CreditDeductionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SwitchCollectionHistories::CollectionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SwitchCollectionHistories::Signature)
                            .text()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SwitchCollectionHistories::Status)
                            .custom(CreationStatus::Type)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SwitchCollectionHistories::InitiatedBy)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SwitchCollectionHistories::CreatedAt)
                            .timestamp()
                            .not_null()
                            .extra("default now()".to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("switch_collection_histories_collection_mint_id_idx")
                    .table(SwitchCollectionHistories::Table)
                    .col(SwitchCollectionHistories::CollectionMintId)
                    .index_type(IndexType::BTree)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("switch_collection_histories_credit_deduction_id_idx")
                    .table(SwitchCollectionHistories::Table)
                    .col(SwitchCollectionHistories::CreditDeductionId)
                    .index_type(IndexType::BTree)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("switch_collection_histories_status_idx")
                    .table(SwitchCollectionHistories::Table)
                    .col(SwitchCollectionHistories::Status)
                    .index_type(IndexType::BTree)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(SwitchCollectionHistories::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum SwitchCollectionHistories {
    Table,
    Id,
    CollectionMintId,
    CollectionId,
    CreditDeductionId,
    Signature,
    Status,
    InitiatedBy,
    CreatedAt,
}
