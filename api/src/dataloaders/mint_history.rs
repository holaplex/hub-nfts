use std::collections::HashMap;

use async_graphql::{dataloader::Loader as DataLoader, FieldError, Result};
use poem::async_trait;
use sea_orm::{prelude::*, JoinType, Order, QueryOrder, QuerySelect};

use crate::{
    db::Connection,
    entities::{collection_mints, mint_history},
};

#[derive(Debug, Clone)]
pub struct CollectionLoader {
    pub db: Connection,
}

impl CollectionLoader {
    #[must_use]
    pub fn new(db: Connection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DataLoader<Uuid> for CollectionLoader {
    type Error = FieldError;
    type Value = Vec<mint_history::Model>;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let conn = self.db.get();
        let mint_history = mint_history::Entity::find()
            .join(
                JoinType::InnerJoin,
                mint_history::Relation::CollectionMints.def(),
            )
            .filter(
                collection_mints::Column::CollectionId.is_in(keys.iter().map(ToOwned::to_owned)),
            )
            .select_also(collection_mints::Entity)
            .order_by(mint_history::Column::CreatedAt, Order::Desc)
            .all(conn)
            .await?;

        Ok(mint_history
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
