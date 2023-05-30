use hub_core::{anyhow::Result, chrono::Utc, clap, prelude::*, producer::Producer};
use mpl_token_metadata::{
    instruction::{mint_new_edition_from_master_edition_via_token, update_metadata_accounts_v2},
    state::{Creator, DataV2, EDITION, PREFIX},
};
use sea_orm::{prelude::*, Set};
use solana_client::rpc_client::RpcClient;
use solana_program::{program_pack::Pack, pubkey::Pubkey, system_instruction::create_account};
use solana_sdk::signer::{keypair::Keypair, Signer};
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account,
};
use spl_token::{
    instruction::{initialize_mint, mint_to},
    state,
};

use super::{Edition, Event, TransactionResponse};
use crate::{
    entities::{
        collection_creators,
        collections::{self, Model as CollectionModel},
        metadata_jsons,
        metadata_jsons::Model as MetadataJsonModel,
        nft_transfers,
        prelude::{CollectionCreators, MetadataJsons},
        solana_collections,
    },
    proto::{
        solana_events::Event::{
            CreateDrop, MintDrop, RetryDrop, RetryMintDrop, TransferAsset, UpdateDrop,
        },
        MetaplexMasterEditionTransaction, MintMetaplexEditionTransaction, NftEventKey,
        SolanaEvents, TransferMetaplexAssetTransaction,
    },
};

#[derive(Clone)]
pub struct Solana {
    producer: Producer<SolanaEvents>,
}

impl Solana {
    pub fn new(producer: Producer<SolanaEvents>) -> Self {
        Self { producer }
    }

    pub fn event(
        &self,
    ) -> impl Event<
        MetaplexMasterEditionTransaction,
        MintMetaplexEditionTransaction,
        TransferMetaplexAssetTransaction,
    > {
        self.clone()
    }
}

#[async_trait::async_trait]
impl
    Event<
        MetaplexMasterEditionTransaction,
        MintMetaplexEditionTransaction,
        TransferMetaplexAssetTransaction,
    > for Solana
{
    async fn create_drop(
        &self,
        key: NftEventKey,
        payload: MetaplexMasterEditionTransaction,
    ) -> Result<()> {
        let event = SolanaEvents {
            event: Some(CreateDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn retry_create_drop(
        &self,
        key: NftEventKey,
        payload: MetaplexMasterEditionTransaction,
    ) -> Result<()> {
        let event = SolanaEvents {
            event: Some(RetryDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn update_drop(
        &self,
        key: NftEventKey,
        payload: MetaplexMasterEditionTransaction,
    ) -> Result<()> {
        let event = SolanaEvents {
            event: Some(UpdateDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn mint_drop(
        &self,
        key: NftEventKey,
        payload: MintMetaplexEditionTransaction,
    ) -> Result<()> {
        let event = SolanaEvents {
            event: Some(MintDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn retry_mint_drop(
        &self,
        key: NftEventKey,
        payload: MintMetaplexEditionTransaction,
    ) -> Result<()> {
        let event = SolanaEvents {
            event: Some(RetryMintDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn transfer_asset(
        &self,
        key: NftEventKey,
        payload: TransferMetaplexAssetTransaction,
    ) -> Result<()> {
        let event: SolanaEvents = SolanaEvents {
            event: Some(TransferAsset(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }
}

