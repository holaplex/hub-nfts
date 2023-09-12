use std::collections::HashMap;

use async_graphql::{dataloader::Loader as DataLoader, FieldError, Result};
use poem::async_trait;
use sea_orm::{prelude::*, Order, QueryOrder};

use crate::{db::Connection, entities::switch_collection_histories};

#[derive(Debug, Clone)]
pub struct SwitchCollectionHistoryLoader {
    pub db: Connection,
}

impl SwitchCollectionHistoryLoader {
    #[must_use]
    pub fn new(db: Connection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DataLoader<Uuid> for SwitchCollectionHistoryLoader {
    type Error = FieldError;
    type Value = Vec<switch_collection_histories::Model>;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let conn = self.db.get();
        let switch_histories = switch_collection_histories::Entity::find()
            .filter(
                switch_collection_histories::Column::CollectionMintId
                    .is_in(keys.iter().map(ToOwned::to_owned)),
            )
            .order_by(switch_collection_histories::Column::CreatedAt, Order::Desc)
            .all(conn)
            .await?;

        Ok(switch_histories.into_iter().fold(
            HashMap::new(),
            |mut acc: HashMap<Uuid, Vec<switch_collection_histories::Model>>, switch_history| {
                acc.entry(switch_history.collection_mint_id)
                    .or_insert_with(Vec::new);

                acc.entry(switch_history.collection_mint_id)
                    .and_modify(|switch_histories| switch_histories.push(switch_history));

                acc
            },
        ))
    }
}
