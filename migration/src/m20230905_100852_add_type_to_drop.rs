use sea_orm_migration::{prelude::*, sea_query::extension::postgres::Type};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type(
                Type::create()
                    .as_enum(DropType::Type)
                    .values([DropType::Edition, DropType::Open])
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Drops::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Drops::DropType)
                            .custom(DropType::Type)
                            .default("edition")
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Drops::Table)
                    .drop_column(Drops::DropType)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Drops {
    Table,
    DropType,
}

pub enum DropType {
    Type,
    Edition,
    Open,
}

impl Iden for DropType {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(s, "{}", match self {
            Self::Type => "drop_type",
            Self::Edition => "edition",
            Self::Open => "open",
        })
        .unwrap();
    }
}
