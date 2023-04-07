use std::collections::HashMap;

use async_graphql::{dataloader::Loader as DataLoader, FieldError, Result};
use poem::async_trait;
use sea_orm::{prelude::*, JoinType, Order, QueryOrder, QuerySelect};

use crate::{
    db::Connection,
    entities::{collection_mints, drops, purchases},
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
    type Value = Vec<purchases::Model>;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let conn = self.db.get();
        let purchases = purchases::Entity::find()
            .join(
                JoinType::InnerJoin,
                purchases::Relation::CollectionMints.def(),
            )
            .filter(
                collection_mints::Column::CollectionId.is_in(keys.iter().map(ToOwned::to_owned)),
            )
            .select_also(collection_mints::Entity)
            .order_by(purchases::Column::CreatedAt, Order::Desc)
            .all(conn)
            .await?;

        Ok(purchases
            .into_iter()
            .fold(HashMap::new(), |mut acc, (purchase, collection_mint)| {
                if let Some(collection_mint) = collection_mint {
                    acc.entry(collection_mint.collection_id)
                        .or_insert_with(Vec::new);

                    acc.entry(collection_mint.collection_id)
                        .and_modify(|purchases| purchases.push(purchase));

                    acc
                } else {
                    acc
                }
            }))
    }
}

#[derive(Debug, Clone)]
pub struct DropLoader {
    pub db: Connection,
}

impl DropLoader {
    #[must_use]
    pub fn new(db: Connection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DataLoader<Uuid> for DropLoader {
    type Error = FieldError;
    type Value = Vec<purchases::Model>;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let conn = self.db.get();
        let purchases = drops::Entity::find()
            .join(JoinType::InnerJoin, drops::Relation::Purchases.def())
            .filter(purchases::Column::DropId.is_in(keys.iter().map(ToOwned::to_owned)))
            .select_with(purchases::Entity)
            .order_by(purchases::Column::CreatedAt, Order::Desc)
            .all(conn)
            .await?;

        Ok(purchases
            .into_iter()
            .map(|(drop, purchases)| (drop.id, purchases))
            .collect())
    }
}
