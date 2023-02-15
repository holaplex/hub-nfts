pub use sea_orm_migration::prelude::*;

mod m20230208_205152_create_solana_collections_table;
mod m20230209_052025_create_drops_table;
mod m20230209_052038_create_collection_attributes_table;
mod m20230209_052046_create_collection_mints_table;
mod m20230214_212301_create_collections_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20230208_205152_create_solana_collections_table::Migration),
            Box::new(m20230209_052025_create_drops_table::Migration),
            Box::new(m20230209_052038_create_collection_attributes_table::Migration),
            Box::new(m20230209_052046_create_collection_mints_table::Migration),
            Box::new(m20230214_212301_create_collections_table::Migration),
        ]
    }
}
