use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CollectionAttributes::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CollectionAttributes::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("default gen_random_uuid()".to_string()),
                    )
                    .col(
                        ColumnDef::new(CollectionAttributes::CollectionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CollectionAttributes::TraitType)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CollectionAttributes::Value)
                            .text()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                IndexCreateStatement::new()
                    .name("collection_attributes_unique_index")
                    .table(CollectionAttributes::Table)
                    .col(CollectionAttributes::TraitType)
                    .col(CollectionAttributes::Value)
                    .unique()
                    .index_type(IndexType::BTree)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CollectionAttributes::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum CollectionAttributes {
    Table,
    Id,
    CollectionId,
    TraitType,
    Value,
}
