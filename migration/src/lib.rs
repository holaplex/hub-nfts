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
mod m20230303_235929_create_project_wallets_table;
mod m20230304_001527_add_ethereum_to_blockchain_type;
mod m20230304_112047_rename_collection_attributes_to_metadata_json_attributes;
mod m20230304_121614_move_collections_columns_to_metadata_jsons;
mod m20230305_154732_drop_created_by_on_solana_collections;
mod m20230306_100027_add_address_to_collections;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20230209_052038_create_collection_attributes_table::Migration),
            Box::new(m20230214_212301_create_collections_table::Migration),
            Box::new(m20230215_194724_create_drops_table::Migration),
            Box::new(m20230220_223223_create_collection_mints_table::Migration),
            Box::new(m20230223_145645_create_solana_collections_table::Migration),
            Box::new(m20230303_155836_add_metadata_json_tables::Migration),
            Box::new(m20230303_171740_create_collection_attributes_table::Migration),
            Box::new(m20230303_171749_create_metadata_json_files::Migration),
            Box::new(m20230303_171755_create_creators_table::Migration),
            Box::new(m20230303_235929_create_project_wallets_table::Migration),
            Box::new(m20230304_001527_add_ethereum_to_blockchain_type::Migration),
            Box::new(m20230304_112047_rename_collection_attributes_to_metadata_json_attributes::Migration),
            Box::new(m20230304_121614_move_collections_columns_to_metadata_jsons::Migration),
            Box::new(m20230305_154732_drop_created_by_on_solana_collections::Migration),
            Box::new(m20230306_100027_add_address_to_collections::Migration),
        ]
    }
}
