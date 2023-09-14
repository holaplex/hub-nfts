use hub_core::{anyhow::Result, prelude::bail, producer::Producer};

use super::{DropEvent, TransferEvent};
use crate::{
    entities::sea_orm_active_enums::DropType,
    proto::{
        nft_events::Event::{
            PolygonCreateDrop, PolygonMintDrop, PolygonRetryDrop, PolygonRetryMintDrop,
            PolygonTransferAsset, PolygonUpdateDrop,
        },
        CreateEditionTransaction, MintEditionTransaction, NftEventKey, NftEvents,
        TransferPolygonAsset, UpdateEdtionTransaction,
    },
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
    ) -> impl DropEvent<CreateEditionTransaction, MintEditionTransaction, UpdateEdtionTransaction>
    + TransferEvent<TransferPolygonAsset> {
        self.clone()
    }
}

#[async_trait::async_trait]
impl DropEvent<CreateEditionTransaction, MintEditionTransaction, UpdateEdtionTransaction>
    for Polygon
{
    async fn create_drop(
        &self,
        drop_type: DropType,
        key: NftEventKey,
        payload: CreateEditionTransaction,
    ) -> Result<()> {
        let event = match drop_type {
            DropType::Edition => Some(PolygonCreateDrop(payload)),
            DropType::Open => bail!("Open drops are not supported on Polygon"),
        };

        self.producer
            .send(Some(&NftEvents { event }), Some(&key))
            .await?;

        Ok(())
    }

    async fn retry_create_drop(
        &self,
        drop_type: DropType,
        key: NftEventKey,
        payload: CreateEditionTransaction,
    ) -> Result<()> {
        let event = match drop_type {
            DropType::Edition => Some(PolygonRetryDrop(payload)),
            DropType::Open => bail!("Open drops are not supported on Polygon"),
        };

        self.producer
            .send(Some(&NftEvents { event }), Some(&key))
            .await?;

        Ok(())
    }

    async fn update_drop(
        &self,
        drop_type: DropType,
        key: NftEventKey,
        payload: UpdateEdtionTransaction,
    ) -> Result<()> {
        let event = match drop_type {
            DropType::Edition => Some(PolygonUpdateDrop(payload)),
            DropType::Open => bail!("Open drops are not supported on Polygon"),
        };

        self.producer
            .send(Some(&NftEvents { event }), Some(&key))
            .await?;

        Ok(())
    }

    async fn mint_drop(&self, key: NftEventKey, payload: MintEditionTransaction) -> Result<()> {
        self.producer
            .send(
                Some(&NftEvents {
                    event: Some(PolygonMintDrop(payload)),
                }),
                Some(&key),
            )
            .await?;

        Ok(())
    }

    async fn retry_mint_drop(
        &self,
        key: NftEventKey,
        payload: MintEditionTransaction,
    ) -> Result<()> {
        self.producer
            .send(
                Some(&NftEvents {
                    event: Some(PolygonRetryMintDrop(payload)),
                }),
                Some(&key),
            )
            .await?;

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
