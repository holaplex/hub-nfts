pub use sea_orm_migration::prelude::*;

mod m20230209_052038_create_collection_attributes_table;
mod m20230214_212301_create_collections_table;
mod m20230215_194724_create_drops_table;
mod m20230220_223223_create_collection_mints_table;
mod m20230223_145645_create_solana_collections_table;
mod m20230303_155836_add_metadata_json_tables;
mod m20230303_171740_create_collection_attributes_table;
mod m20230303_171749_create_metadata_json_files;
mod m20230303_171755_create_creators_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20230214_212301_create_collections_table::Migration),
            Box::new(m20230215_194724_create_drops_table::Migration),
            Box::new(m20230220_223223_create_collection_mints_table::Migration),
            Box::new(m20230209_052038_create_collection_attributes_table::Migration),
            Box::new(m20230223_145645_create_solana_collections_table::Migration),
            Box::new(m20230303_155836_add_metadata_json_tables::Migration),
            Box::new(m20230303_171740_create_collection_attributes_table::Migration),
            Box::new(m20230303_171749_create_metadata_json_files::Migration),
            Box::new(m20230303_171755_create_creators_table::Migration),
        ]
    }
}
