use hub_core::{anyhow::Result, producer::Producer};

use super::{DropEvent, TransferEvent};
use crate::proto::{
    nft_events::Event::{
        PolygonCreateDrop, PolygonMintDrop, PolygonRetryDrop, PolygonRetryMintDrop,
        PolygonTransferAsset, PolygonUpdateDrop,
    },
    CreateEditionTransaction, MintEditionTransaction, NftEventKey, NftEvents, TransferPolygonAsset,
    UpdateEdtionTransaction,
};

#[derive(Clone)]
pub struct Polygon {
    producer: Producer<NftEvents>,
}

impl Polygon {
    #[must_use]
    pub fn new(producer: Producer<NftEvents>) -> Self {
        Self { producer }
    }

    #[must_use]
    pub fn event(
        &self,
    ) -> impl DropEvent<
        CreateEditionTransaction,
        MintEditionTransaction,
        UpdateEdtionTransaction,
    > + TransferEvent<TransferPolygonAsset> {
        self.clone()
    }
}

#[async_trait::async_trait]
impl
    DropEvent<
        CreateEditionTransaction,
        MintEditionTransaction,
        UpdateEdtionTransaction,
    > for Polygon
{
    async fn create_drop(&self, key: NftEventKey, payload: CreateEditionTransaction) -> Result<()> {
        let event = NftEvents {
            event: Some(PolygonCreateDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn retry_create_drop(
        &self,
        key: NftEventKey,
        payload: CreateEditionTransaction,
    ) -> Result<()> {
        let event = NftEvents {
            event: Some(PolygonRetryDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn update_drop(&self, key: NftEventKey, payload: UpdateEdtionTransaction) -> Result<()> {
        let event = NftEvents {
            event: Some(PolygonUpdateDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn mint_drop(&self, key: NftEventKey, payload: MintEditionTransaction) -> Result<()> {
        let event = NftEvents {
            event: Some(PolygonMintDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn retry_mint_drop(
        &self,
        key: NftEventKey,
        payload: MintEditionTransaction,
    ) -> Result<()> {
        let event = NftEvents {
            event: Some(PolygonRetryMintDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl TransferEvent<TransferPolygonAsset> for Polygon {
    async fn transfer_asset(&self, key: NftEventKey, payload: TransferPolygonAsset) -> Result<()> {
        let event = NftEvents {
            event: Some(PolygonTransferAsset(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }
}
