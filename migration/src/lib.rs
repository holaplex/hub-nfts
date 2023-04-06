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
mod m20230306_131824_make_end_time_nullable_on_drops;
mod m20230306_132259_default_start_time_on_drops;
mod m20230306_151630_start_time_nullable_on_drops;
mod m20230306_154554_drop_unique_identifier_index_on_metadata_jsons;
mod m20230306_160517_add_total_mints_to_collections;
mod m20230327_114558_add_paused_at_column_to_drops;
mod m20230327_194951_add_shutdown_at_column_to_drops_table;
mod m20230328_212141_add_signature_column_to_collections_n_mints;
mod m20230328_213529_add_more_creation_status_events;
mod m20230405_133333_rename_collection_id_to_id_and_rm_fk;

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
            Box::new(m20230306_131824_make_end_time_nullable_on_drops::Migration),
            Box::new(m20230306_132259_default_start_time_on_drops::Migration),
            Box::new(m20230306_151630_start_time_nullable_on_drops::Migration),
            Box::new(m20230306_154554_drop_unique_identifier_index_on_metadata_jsons::Migration),
            Box::new(m20230306_160517_add_total_mints_to_collections::Migration),
            Box::new(m20230327_114558_add_paused_at_column_to_drops::Migration),
            Box::new(m20230327_194951_add_shutdown_at_column_to_drops_table::Migration),
            Box::new(m20230328_212141_add_signature_column_to_collections_n_mints::Migration),
            Box::new(m20230328_213529_add_more_creation_status_events::Migration),
            Box::new(m20230405_133333_rename_collection_id_to_id_and_rm_fk::Migration),
        ]
    }
}
