use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(JobTrackings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(JobTrackings::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(JobTrackings::JobType).string().not_null())
                    .col(ColumnDef::new(JobTrackings::Status).string().not_null())
                    .col(ColumnDef::new(JobTrackings::Payload).json().not_null())
                    .col(
                        ColumnDef::new(JobTrackings::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(JobTrackings::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(JobTrackings::Table).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum JobTrackings {
    Table,
    Id,
    Status,
    JobType,
    Payload,
    CreatedAt,
    UpdatedAt,
}
