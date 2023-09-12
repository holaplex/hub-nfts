use sea_orm_migration::{prelude::*, sea_query::extension::postgres::Type};

use crate::m20230303_155836_add_metadata_json_tables::MetadataJsons;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type(
                Type::create()
                    .as_enum(MetadataJsonJobType::Type)
                    .values([MetadataJsonJobType::Upload, MetadataJsonJobType::Download])
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MetadataJsonJobs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MetadataJsonJobs::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("default gen_random_uuid()".to_owned()),
                    )
                    .col(
                        ColumnDef::new(MetadataJsonJobs::Type)
                            .custom(MetadataJsonJobType::Type)
                            .not_null(),
                    )
                    .col(ColumnDef::new(MetadataJsonJobs::Continuation).binary().null())
                    .col(ColumnDef::new(MetadataJsonJobs::Failed).boolean().not_null().default(false))
                    .col(ColumnDef::new(MetadataJsonJobs::Url).string().null())
                    .col(ColumnDef::new(MetadataJsonJobs::MetadataJsonId).uuid().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-metadata_json_jobs_metadata_json_id")
                            .from(MetadataJsonJobs::Table, MetadataJsonJobs::MetadataJsonId)
                            .to(MetadataJsons::Table, Alias::new("id")) // ???
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MetadataJsonJobs::Table).to_owned())
            .await?;

        manager
            .drop_type(Type::drop().name(MetadataJsonJobType::Type).to_owned())
            .await
    }
}

#[derive(Iden)]
enum MetadataJsonJobs {
    Table,
    Id,
    Type,
    Continuation,
    Failed,
    Url,
    MetadataJsonId,
}

pub enum MetadataJsonJobType {
    Type,
    Upload,
    Download,
}

impl Iden for MetadataJsonJobType {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        s.write_str(match self {
            Self::Type => "metadata_json_job_type",
            Self::Upload => "upload",
            Self::Download => "download",
        })
        .unwrap()
    }
}
