use std::{borrow::Cow, collections::HashMap};

use async_graphql::{dataloader::Loader as DataLoader, FieldError, Result};
use poem::async_trait;
use sea_orm::{prelude::*, JoinType, QuerySelect};

use crate::{
    db::Connection,
    entities::{collection_mints, collections, drops, sea_orm_active_enums::CreationStatus},
    objects::CollectionMint,
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
    type Value = Vec<CollectionMint>;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let collection_mints = collection_mints::Entity::find()
            .filter(
                collection_mints::Column::CollectionId.is_in(keys.iter().map(ToOwned::to_owned)),
            )
            .filter(collection_mints::Column::CreationStatus.ne(CreationStatus::Queued))
            .all(self.db.get())
            .await?;

        Ok(collection_mints
            .into_iter()
            .fold(HashMap::new(), |mut acc, collection_mint| {
                acc.entry(collection_mint.collection_id)
                    .or_insert_with(Vec::new);

                acc.entry(collection_mint.collection_id)
                    .and_modify(|collection_mints| collection_mints.push(collection_mint.into()));

                acc
            }))
    }
}

#[derive(Debug, Clone)]
pub struct OwnerLoader {
    pub db: Connection,
}

impl OwnerLoader {
    #[must_use]
    pub fn new(db: Connection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DataLoader<String> for OwnerLoader {
    type Error = FieldError;
    type Value = Vec<CollectionMint>;

    async fn load(&self, keys: &[String]) -> Result<HashMap<String, Self::Value>, Self::Error> {
        let collection_mints = collection_mints::Entity::find()
            .filter(
                collection_mints::Column::Owner
                    .is_in(hub_core::util::downcase_evm_addresses(keys).map(Cow::into_owned)),
            )
            .all(self.db.get())
            .await?;

        Ok(collection_mints
            .into_iter()
            .fold(HashMap::new(), |mut acc, collection_mint| {
                if let Some(owner) = collection_mint.owner.clone() {
                    acc.entry(owner)
                        .or_insert_with(Vec::new)
                        .push(collection_mint.into());
                }
                acc
            }))
    }
}

#[derive(Debug, Clone)]
pub struct CollectionMintLoader {
    pub db: Connection,
}

impl CollectionMintLoader {
    #[must_use]
    pub fn new(db: Connection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DataLoader<Uuid> for CollectionMintLoader {
    type Error = FieldError;
    type Value = CollectionMint;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let collection_mints = collection_mints::Entity::find()
            .filter(collection_mints::Column::Id.is_in(keys.iter().map(ToOwned::to_owned)))
            .all(self.db.get())
            .await?;

        Ok(collection_mints
            .into_iter()
            .map(|collection_mint| (collection_mint.id, collection_mint.into()))
            .collect())
    }
}

/// Dataloader for queued mints for a drop
#[derive(Debug, Clone)]
pub struct QueuedMintsLoader {
    pub db: Connection,
}

impl QueuedMintsLoader {
    #[must_use]
    pub fn new(db: Connection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DataLoader<Uuid> for QueuedMintsLoader {
    type Error = FieldError;
    type Value = Vec<CollectionMint>;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let drop_mints = drops::Entity::find()
            .select_with(collection_mints::Entity)
            .join(JoinType::InnerJoin, drops::Relation::Collections.def())
            .join(
                JoinType::InnerJoin,
                collections::Relation::CollectionMints.def(),
            )
            .filter(collection_mints::Column::CreationStatus.eq(CreationStatus::Queued))
            .filter(drops::Column::Id.is_in(keys.iter().map(ToOwned::to_owned)))
            .all(self.db.get())
            .await?;

        Ok(drop_mints
            .into_iter()
            .map(|(drop, mints)| (drop.id, mints.into_iter().map(Into::into).collect()))
            .collect())
    }
}
