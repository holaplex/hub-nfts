use hub_core::{anyhow::Result, producer::Producer};

use super::Event;
use crate::proto::{
    nft_events::Event::{
        SolanaCreateDrop, SolanaMintDrop, SolanaRetryDrop, SolanaRetryMintDrop,
        SolanaTransferAsset, SolanaUpdateDrop,
    },
    MetaplexMasterEditionTransaction, MintMetaplexEditionTransaction, NftEventKey, NftEvents,
    TransferMetaplexAssetTransaction,
};

#[derive(Clone)]
pub struct Solana {
    producer: Producer<NftEvents>,
}

impl Solana {
    #[must_use]
    pub fn new(producer: Producer<NftEvents>) -> Self {
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
        let event = NftEvents {
            event: Some(SolanaCreateDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn retry_create_drop(
        &self,
        key: NftEventKey,
        payload: MetaplexMasterEditionTransaction,
    ) -> Result<()> {
        let event = NftEvents {
            event: Some(SolanaRetryDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn update_drop(
        &self,
        key: NftEventKey,
        payload: MetaplexMasterEditionTransaction,
    ) -> Result<()> {
        let event = NftEvents {
            event: Some(SolanaUpdateDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn mint_drop(
        &self,
        key: NftEventKey,
        payload: MintMetaplexEditionTransaction,
    ) -> Result<()> {
        let event = NftEvents {
            event: Some(SolanaMintDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn retry_mint_drop(
        &self,
        key: NftEventKey,
        payload: MintMetaplexEditionTransaction,
    ) -> Result<()> {
        let event = NftEvents {
            event: Some(SolanaRetryMintDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn transfer_asset(
        &self,
        key: NftEventKey,
        payload: TransferMetaplexAssetTransaction,
    ) -> Result<()> {
        let event: NftEvents = NftEvents {
            event: Some(SolanaTransferAsset(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }
}
