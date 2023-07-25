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
mod m20230406_164930_create_purchase_history_table;
mod m20230411_230029_create_nft_transfers_table;
mod m20230412_073127_create_column_seller_fee_basis_points_on_collections;
mod m20230412_144354_backfill_collections_seller_fee_basis_points_from_solana_collections;
mod m20230412_150236_drop_seller_fee_basis_points_on_solana_collections;
mod m20230414_110500_create_column_seller_fee_basis_points_on_collection_mints;
mod m20230414_110554_backfill_collection_miint_seller_fee_basis_points_from_solana_collections;
mod m20230415_202208_add_edition_field_to_collection_mints;
mod m20230501_121532_add_credits_deduction_id_to_collection_mints;
mod m20230501_121545_add_credits_deduction_id_to_drops;
mod m20230501_130939_add_credits_deduction_id_to_nft_transfers_table;
mod m20230510_160600_change_datatype_to_tz_utc;
mod m20230518_034021_set_default_timestamp;
mod m20230606_121315_add_collection_mint_id_to_nft_transfers;
mod m20230620_160452_make_address_nullable_on_collection_mints;
mod m20230626_111748_customer_wallets_table;
mod m20230706_130934_create_transfer_charges_table;
mod m20230706_133356_backfill_transfer_charges;
mod m20230706_134402_drop_column_credits_deduction_id_from_nft_transfers;
mod m20230706_142939_add_columns_project_id_and_credits_deduction_id_to_collections;
mod m20230713_151414_create_mint_creators_table;
mod m20230713_163043_add_column_compressed_to_collection_mints;
mod m20230718_111347_add_created_at_and_created_by_columns_to_collections;
mod m20230725_135946_rename_purchases_to_mint_history;
mod m20230725_144506_drop_solana_collections_table;

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
            Box::new(m20230406_164930_create_purchase_history_table::Migration),
            Box::new(m20230411_230029_create_nft_transfers_table::Migration),
            Box::new(m20230412_073127_create_column_seller_fee_basis_points_on_collections::Migration),
            Box::new(m20230412_144354_backfill_collections_seller_fee_basis_points_from_solana_collections::Migration),
            Box::new(m20230412_150236_drop_seller_fee_basis_points_on_solana_collections::Migration),
            Box::new(m20230414_110500_create_column_seller_fee_basis_points_on_collection_mints::Migration),
            Box::new(m20230414_110554_backfill_collection_miint_seller_fee_basis_points_from_solana_collections::Migration),
            Box::new(m20230415_202208_add_edition_field_to_collection_mints::Migration),
            Box::new(m20230501_121532_add_credits_deduction_id_to_collection_mints::Migration),
            Box::new(m20230501_121545_add_credits_deduction_id_to_drops::Migration),
            Box::new(m20230501_130939_add_credits_deduction_id_to_nft_transfers_table::Migration),
            Box::new(m20230510_160600_change_datatype_to_tz_utc::Migration),
            Box::new(m20230518_034021_set_default_timestamp::Migration),
            Box::new(m20230606_121315_add_collection_mint_id_to_nft_transfers::Migration),
            Box::new(m20230620_160452_make_address_nullable_on_collection_mints::Migration),
            Box::new(m20230626_111748_customer_wallets_table::Migration),
            Box::new(m20230706_130934_create_transfer_charges_table::Migration),
            Box::new(m20230706_133356_backfill_transfer_charges::Migration),
            Box::new(m20230706_134402_drop_column_credits_deduction_id_from_nft_transfers::Migration),
            Box::new(m20230706_142939_add_columns_project_id_and_credits_deduction_id_to_collections::Migration),
            Box::new(m20230713_151414_create_mint_creators_table::Migration),
            Box::new(m20230713_163043_add_column_compressed_to_collection_mints::Migration),
            Box::new(m20230718_111347_add_created_at_and_created_by_columns_to_collections::Migration),
            Box::new(m20230725_135946_rename_purchases_to_mint_history::Migration),
            Box::new(m20230725_144506_drop_solana_collections_table::Migration),
        ]
    }
}
