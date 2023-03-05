use std::collections::HashMap;

use async_graphql::{dataloader::Loader as DataLoader, FieldError, Result};
use poem::async_trait;
use sea_orm::prelude::*;

use crate::{db::Connection, entities::collection_mints};

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
    type Value = Vec<collection_mints::Model>;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let collection_mints = collection_mints::Entity::find()
            .filter(
                collection_mints::Column::CollectionId.is_in(keys.iter().map(ToOwned::to_owned)),
            )
            .all(self.db.get())
            .await?;

        Ok(collection_mints
            .into_iter()
            .fold(HashMap::new(), |mut acc, collection_mint| {
                acc.entry(collection_mint.collection_id)
                    .or_insert_with(Vec::new);

                acc.entry(collection_mint.collection_id)
                    .and_modify(|attributes| attributes.push(collection_mint));

                acc
            }))
    }
}
