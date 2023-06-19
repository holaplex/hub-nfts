use hub_core::{anyhow::Result, producer::Producer};

use super::Event;
use crate::proto::{
    solana_events::Event::{
        CreateDrop, MintDrop, RetryDrop, RetryMintDrop, TransferAsset, UpdateDrop,
    },
    MetaplexMasterEditionTransaction, MintMetaplexEditionTransaction, NftEventKey, SolanaEvents,
    TransferMetaplexAssetTransaction,
};

#[derive(Clone)]
pub struct Solana {
    producer: Producer<SolanaEvents>,
}

impl Solana {
    #[must_use]
    pub fn new(producer: Producer<SolanaEvents>) -> Self {
        Self { producer }
    }

    #[must_use]
    pub fn event(
        &self,
    ) -> impl Event<
        MetaplexMasterEditionTransaction,
        MintMetaplexEditionTransaction,
        TransferMetaplexAssetTransaction,
        MetaplexMasterEditionTransaction,
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
        MetaplexMasterEditionTransaction,
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
