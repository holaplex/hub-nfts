use std::collections::HashMap;

use async_graphql::{dataloader::Loader as DataLoader, FieldError, Result};
use poem::async_trait;
use sea_orm::{prelude::*, Order, QueryOrder};

use crate::{db::Connection, entities::nft_transfers};

#[derive(Debug, Clone)]
pub struct CollectionMintTransfersLoader {
    pub db: Connection,
}

impl CollectionMintTransfersLoader {
    #[must_use]
    pub fn new(db: Connection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DataLoader<Uuid> for CollectionMintTransfersLoader {
    type Error = FieldError;
    type Value = Vec<nft_transfers::Model>;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let conn = self.db.get();
        let nft_transfers = nft_transfers::Entity::find()
            .filter(
                nft_transfers::Column::CollectionMintId.is_in(keys.iter().map(ToOwned::to_owned)),
            )
            .order_by(nft_transfers::Column::CreatedAt, Order::Desc)
            .all(conn)
            .await?;

        Ok(nft_transfers
            .into_iter()
            .fold(HashMap::new(), |mut acc, nft_transfer| {
                acc.entry(nft_transfer.collection_mint_id)
                    .or_insert_with(Vec::new);

                acc.entry(nft_transfer.collection_mint_id)
                    .and_modify(|nft_transfers| nft_transfers.push(nft_transfer));

                acc
            }))
    }
}
