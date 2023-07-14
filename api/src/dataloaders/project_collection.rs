use std::collections::HashMap;

use async_graphql::{dataloader::Loader as DataLoader, FieldError, Result};
use poem::async_trait;
use sea_orm::prelude::*;

use crate::{db::Connection, entities::collections, objects::Collection};

#[derive(Debug, Clone)]
pub struct ProjectLoader {
    pub db: Connection,
}

impl ProjectLoader {
    #[must_use]
    pub fn new(db: Connection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DataLoader<Uuid> for ProjectLoader {
    type Error = FieldError;
    type Value = Vec<Collection>;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let collections = collections::Entity::find()
            .filter(collections::Column::ID.is_in(keys.iter().map(ToOwned::to_owned)))
            .all(self.db.get())
            .await?;

        Ok(collections
            .into_iter()
            .map(|collection| Ok((collection.id, collection.into())))
            .collect()
        )
    }
}
