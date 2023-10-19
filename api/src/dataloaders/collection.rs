use std::collections::HashMap;

use async_graphql::{dataloader::Loader as DataLoader, FieldError, Result};
use poem::async_trait;
use redis::{AsyncCommands, Client as Redis};
use sea_orm::{prelude::*, FromQueryResult, QueryFilter, QuerySelect};

use crate::{
    db::Connection,
    entities::{
        collection_mints, collections, drops,
        sea_orm_active_enums::{CreationStatus, DropType},
    },
    objects::Collection,
};

#[derive(Debug, Clone)]
pub struct Loader {
    pub db: Connection,
}

impl Loader {
    #[must_use]
    pub fn new(db: Connection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DataLoader<Uuid> for Loader {
    type Error = FieldError;
    type Value = Collection;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let collections = collections::Entity::find()
            .filter(collections::Column::Id.is_in(keys.iter().map(ToOwned::to_owned)))
            .all(self.db.get())
            .await?;

        collections
            .into_iter()
            .map(|collection| Ok((collection.id, collection.into())))
            .collect()
    }
}

#[derive(FromQueryResult, Debug, Clone)]
struct CollectionTotalMintsCount {
    id: Uuid,
    count: i64,
}

#[derive(Debug, Clone)]
pub struct TotalMintsLoader {
    pub db: Connection,
    pub redis: Redis,
}

impl TotalMintsLoader {
    #[must_use]
    pub fn new(db: Connection, redis: Redis) -> Self {
        Self { db, redis }
    }
}

#[async_trait]
impl DataLoader<Uuid> for TotalMintsLoader {
    type Error = FieldError;
    type Value = i64;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let mut results: HashMap<Uuid, Self::Value> = HashMap::new();
        let mut missing_keys: Vec<Uuid> = Vec::new();

        let mut redis_connection = self.redis.get_async_connection().await?;

        for key in keys {
            let redis_key = format!("collection:{key}:total_mints");

            match redis_connection.get::<_, i64>(&redis_key).await {
                Ok(value) => {
                    results.insert(*key, value);
                },
                Err(_) => {
                    missing_keys.push(*key);
                },
            }
        }

        if missing_keys.is_empty() {
            return Ok(results);
        }

        let conn = self.db.get();
        let count_results = collection_mints::Entity::find()
            .select_only()
            .column_as(collection_mints::Column::Id.count(), "count")
            .column_as(collection_mints::Column::CollectionId, "id")
            .filter(
                collection_mints::Column::CollectionId
                    .is_in(missing_keys.iter().map(ToOwned::to_owned))
                    .and(collection_mints::Column::CreationStatus.ne(CreationStatus::Queued)),
            )
            .group_by(collection_mints::Column::CollectionId)
            .into_model::<CollectionTotalMintsCount>()
            .all(conn)
            .await?;
        let count_results = count_results
            .into_iter()
            .map(|result| (result.id, result.count))
            .collect::<HashMap<_, _>>();

        for key in missing_keys {
            let count = count_results.get(&key).copied().unwrap_or_default();
            let redis_key = format!("collection:{key}:total_mints");

            redis_connection.set(&redis_key, count).await?;

            results.insert(key, count);
        }

        Ok(results)
    }
}

#[derive(FromQueryResult)]
struct CollectionSupplyCount {
    id: Uuid,
    count: i64,
}

#[derive(Debug, Clone)]
pub struct SupplyLoader {
    pub db: Connection,
    pub redis: Redis,
}

impl SupplyLoader {
    #[must_use]
    pub fn new(db: Connection, redis: Redis) -> Self {
        Self { db, redis }
    }
}

#[async_trait]
impl DataLoader<Uuid> for SupplyLoader {
    type Error = FieldError;
    type Value = Option<i64>;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let mut results: HashMap<Uuid, Self::Value> = HashMap::new();
        let mut missing_keys: Vec<Uuid> = Vec::new();
        let mut compute_keys: Vec<Uuid> = Vec::new();

        let mut redis_connection = self.redis.get_async_connection().await?;
        let conn = self.db.get();

        for key in keys {
            let redis_key = format!("collection:{key}:supply");

            match redis_connection.get::<_, i64>(&redis_key).await {
                Ok(value) => {
                    results.insert(*key, Some(value));
                },
                Err(_) => {
                    missing_keys.push(*key);
                },
            }
        }

        let collection_with_drops = collections::Entity::find()
            .filter(collections::Column::Id.is_in(missing_keys.iter().map(ToOwned::to_owned)))
            .inner_join(drops::Entity)
            .select_also(drops::Entity)
            .all(conn)
            .await?;

        for (collection, drop) in collection_with_drops {
            match drop {
                Some(drop) if drop.drop_type == DropType::Edition => {
                    results.insert(collection.id, collection.supply);
                },
                Some(_) | None => {
                    compute_keys.push(collection.id);
                },
            }
        }

        if compute_keys.is_empty() {
            return Ok(results);
        }

        let count_results = collection_mints::Entity::find()
            .select_only()
            .column_as(collection_mints::Column::Id.count(), "count")
            .column_as(collection_mints::Column::CollectionId, "id")
            .filter(
                collection_mints::Column::CollectionId
                    .is_in(compute_keys.iter().map(ToOwned::to_owned)),
            )
            .group_by(collection_mints::Column::CollectionId)
            .into_model::<CollectionSupplyCount>()
            .all(conn)
            .await?
            .into_iter()
            .map(|result| (result.id, result.count))
            .collect::<HashMap<_, _>>();

        for key in compute_keys {
            let count = count_results.get(&key).copied().unwrap_or_default();
            let redis_key = format!("collection:{key}:supply");

            redis_connection.set(&redis_key, count).await?;

            results.insert(key, Some(count));
        }

        Ok(results)
    }
}
