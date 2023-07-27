use std::collections::HashMap;

use async_graphql::{dataloader::Loader as DataLoader, FieldError, Result};
use poem::async_trait;
use sea_orm::{prelude::*, JoinType, Order, QueryOrder, QuerySelect};

use crate::{
    db::Connection,
    entities::{collection_mints, collections, drops, mint_histories},
};

#[derive(Debug, Clone)]
pub struct CollectionMintHistoryLoader {
    pub db: Connection,
}

impl CollectionMintHistoryLoader {
    #[must_use]
    pub fn new(db: Connection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DataLoader<Uuid> for CollectionMintHistoryLoader {
    type Error = FieldError;
    type Value = Vec<mint_histories::Model>;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let conn = self.db.get();
        let mint_histories = mint_histories::Entity::find()
            .join(
                JoinType::InnerJoin,
                mint_histories::Relation::CollectionMints.def(),
            )
            .filter(
                collection_mints::Column::CollectionId.is_in(keys.iter().map(ToOwned::to_owned)),
            )
            .select_also(collection_mints::Entity)
            .order_by(mint_histories::Column::CreatedAt, Order::Desc)
            .all(conn)
            .await?;

        Ok(mint_histories
            .into_iter()
            .fold(HashMap::new(), |mut acc, (r, collection_mint)| {
                if let Some(collection_mint) = collection_mint {
                    acc.entry(collection_mint.collection_id)
                        .or_insert_with(Vec::new);

                    acc.entry(collection_mint.collection_id)
                        .and_modify(|mint_history| mint_history.push(r));

                    acc
                } else {
                    acc
                }
            }))
    }
}

#[derive(Debug, Clone)]
pub struct DropMintHistoryLoader {
    pub db: Connection,
}

impl DropMintHistoryLoader {
    #[must_use]
    pub fn new(db: Connection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DataLoader<Uuid> for DropMintHistoryLoader {
    type Error = FieldError;
    type Value = Vec<mint_histories::Model>;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let conn = self.db.get();
        let mint_histories = drops::Entity::find()
            .join(JoinType::InnerJoin, drops::Relation::Collections.def())
            .join(
                JoinType::InnerJoin,
                collections::Relation::MintHistories.def(),
            )
            .filter(drops::Column::Id.is_in(keys.iter().map(ToOwned::to_owned)))
            .select_with(mint_histories::Entity)
            .order_by(mint_histories::Column::CreatedAt, Order::Desc)
            .all(conn)
            .await?;

        Ok(mint_histories
            .into_iter()
            .map(|(drop, mint_histories)| (drop.id, mint_histories))
            .collect())
    }
}