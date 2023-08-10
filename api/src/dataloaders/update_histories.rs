use std::collections::HashMap;

use async_graphql::{dataloader::Loader as DataLoader, FieldError, Result};
use poem::async_trait;
use sea_orm::{prelude::*, Order, QueryOrder};

use crate::{db::Connection, entities::update_histories};

#[derive(Debug, Clone)]
pub struct UpdateMintHistoryLoader {
    pub db: Connection,
}

impl UpdateMintHistoryLoader {
    #[must_use]
    pub fn new(db: Connection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DataLoader<Uuid> for UpdateMintHistoryLoader {
    type Error = FieldError;
    type Value = Vec<update_histories::Model>;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let conn = self.db.get();
        let update_histories = update_histories::Entity::find()
            .filter(update_histories::Column::MintId.is_in(keys.iter().map(ToOwned::to_owned)))
            .order_by(update_histories::Column::CreatedAt, Order::Desc)
            .all(conn)
            .await?;

        Ok(update_histories
            .into_iter()
            .fold(HashMap::new(), |mut acc, history| {
                acc.entry(history.mint_id).or_insert_with(Vec::new);

                acc.entry(history.mint_id)
                    .and_modify(|update_histories| update_histories.push(history.into()));

                acc
            }))
    }
}
