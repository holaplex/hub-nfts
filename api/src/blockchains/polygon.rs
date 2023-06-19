use hub_core::{anyhow::Result, producer::Producer};

use super::Event;
use crate::proto::{
    polygon_events::Event::{
        CreateDrop, MintDrop, RetryDrop, RetryMintDrop, TransferAsset, UpdateDrop,
    },
    CreateEditionTransaction, MintEditionTransaction, NftEventKey, PolygonEvents,
    TransferPolygonAsset, UpdateEdtionTransaction,
};

#[derive(Clone)]
pub struct Polygon {
    producer: Producer<PolygonEvents>,
}

impl Polygon {
    #[must_use]
    pub fn new(producer: Producer<PolygonEvents>) -> Self {
        Self { producer }
    }

    #[must_use]
    pub fn event(
        &self,
    ) -> impl Event<
        CreateEditionTransaction,
        MintEditionTransaction,
        TransferPolygonAsset,
        UpdateEdtionTransaction,
    > {
        self.clone()
    }
}

#[async_trait::async_trait]
impl
    Event<
        CreateEditionTransaction,
        MintEditionTransaction,
        TransferPolygonAsset,
        UpdateEdtionTransaction,
    > for Polygon
{
    async fn create_drop(&self, key: NftEventKey, payload: CreateEditionTransaction) -> Result<()> {
        let event = PolygonEvents {
            event: Some(CreateDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn retry_create_drop(
        &self,
        key: NftEventKey,
        payload: CreateEditionTransaction,
    ) -> Result<()> {
        let event = PolygonEvents {
            event: Some(RetryDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn update_drop(&self, key: NftEventKey, payload: UpdateEdtionTransaction) -> Result<()> {
        let event = PolygonEvents {
            event: Some(UpdateDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn mint_drop(&self, key: NftEventKey, payload: MintEditionTransaction) -> Result<()> {
        let event = PolygonEvents {
            event: Some(MintDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn retry_mint_drop(
        &self,
        key: NftEventKey,
        payload: MintEditionTransaction,
    ) -> Result<()> {
        let event = PolygonEvents {
            event: Some(RetryMintDrop(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;

        Ok(())
    }

    async fn transfer_asset(&self, key: NftEventKey, payload: TransferPolygonAsset) -> Result<()> {
        let event = PolygonEvents {
            event: Some(TransferAsset(payload)),
        };

        self.producer.send(Some(&event), Some(&key)).await?;
        Ok(())
    }
}
