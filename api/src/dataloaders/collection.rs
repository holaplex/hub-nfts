use std::collections::HashMap;

use async_graphql::{dataloader::Loader as DataLoader, FieldError, Result};
use poem::async_trait;
use sea_orm::prelude::*;

use crate::{db::Connection, entities::collections, objects::Collection};

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
            .map(|collection| Ok((collection.id, collection.try_into()?)))
            .collect::<Result<HashMap<Uuid, Self::Value>, _>>()
    }
}
