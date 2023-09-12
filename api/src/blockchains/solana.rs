use hub_core::{anyhow::Result, producer::Producer};

use super::{CollectionEvent, DropEvent, TransferEvent};
use crate::proto::{
    nft_events::Event::{
        SolanaCreateCollection, SolanaCreateDrop, SolanaMintDrop, SolanaMintToCollection,
        SolanaRetryCreateCollection, SolanaRetryDrop, SolanaRetryMintDrop,
        SolanaRetryMintToCollection, SolanaRetryUpdatedCollectionMint,
        SolanaSwitchMintCollectionRequested, SolanaTransferAsset, SolanaUpdateCollection,
        SolanaUpdateDrop, SolanaUpdatedCollectionMint,
    },
    MetaplexMasterEditionTransaction, MintMetaplexEditionTransaction,
    MintMetaplexMetadataTransaction, NftEventKey, NftEvents, RetryUpdateSolanaMintPayload,
    SwitchCollectionPayload, TransferMetaplexAssetTransaction, UpdateSolanaMintPayload,
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
    ) -> impl DropEvent<
        MetaplexMasterEditionTransaction,
        MintMetaplexEditionTransaction,
        MetaplexMasterEditionTransaction,
    > + TransferEvent<TransferMetaplexAssetTransaction>
           + CollectionEvent<
        MetaplexMasterEditionTransaction,
        MetaplexMasterEditionTransaction,
        MintMetaplexMetadataTransaction,
    > {
        self.clone()
    }
}

#[async_trait::async_trait]
impl
    DropEvent<
        MetaplexMasterEditionTransaction,
        MintMetaplexEditionTransaction,
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
}

#[async_trait::async_trait]
impl TransferEvent<TransferMetaplexAssetTransaction> for Solana {
    async fn transfer_asset(
        &self,
        key: NftEventKey,
        payload: TransferMetaplexAssetTransaction,
    ) -> Result<()> {
        let event = NftEvents {
            event: Some(SolanaTransferAsset(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl
    CollectionEvent<
        MetaplexMasterEditionTransaction,
        MetaplexMasterEditionTransaction,
        MintMetaplexMetadataTransaction,
    > for Solana
{
    async fn create_collection(
        &self,
        key: NftEventKey,
        payload: MetaplexMasterEditionTransaction,
    ) -> Result<()> {
        let event = NftEvents {
            event: Some(SolanaCreateCollection(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn retry_create_collection(
        &self,
        key: NftEventKey,
        payload: MetaplexMasterEditionTransaction,
    ) -> Result<()> {
        let event = NftEvents {
            event: Some(SolanaRetryCreateCollection(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn update_collection(
        &self,
        key: NftEventKey,
        payload: MetaplexMasterEditionTransaction,
    ) -> Result<()> {
        let event = NftEvents {
            event: Some(SolanaUpdateCollection(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn mint_to_collection(
        &self,
        key: NftEventKey,
        payload: MintMetaplexMetadataTransaction,
    ) -> Result<()> {
        let event = NftEvents {
            event: Some(SolanaMintToCollection(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn retry_mint_to_collection(
        &self,
        key: NftEventKey,
        payload: MintMetaplexMetadataTransaction,
    ) -> Result<()> {
        let event = NftEvents {
            event: Some(SolanaRetryMintToCollection(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn update_collection_mint(
        &self,
        key: NftEventKey,
        payload: UpdateSolanaMintPayload,
    ) -> Result<()> {
        let event = NftEvents {
            event: Some(SolanaUpdatedCollectionMint(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn retry_update_mint(
        &self,
        key: NftEventKey,
        payload: RetryUpdateSolanaMintPayload,
    ) -> Result<()> {
        let event = NftEvents {
            event: Some(SolanaRetryUpdatedCollectionMint(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;
        Ok(())
    }

    async fn switch_collection(
        &self,
        key: NftEventKey,
        payload: SwitchCollectionPayload,
    ) -> Result<()> {
        let event = NftEvents {
            event: Some(SolanaSwitchMintCollectionRequested(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;
        Ok(())
    }
}
