use std::collections::HashMap;

use async_graphql::{dataloader::Loader, FieldError, Result};
use poem::async_trait;
use sea_orm::prelude::*;

use crate::{db::Connection, entities::drops, objects::Drop};

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
impl Loader<Uuid> for DropLoader {
    type Error = FieldError;
    type Value = Drop;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let drops = drops::Entity::find()
            .filter(drops::Column::Id.is_in(keys.iter().map(ToOwned::to_owned)))
            .all(self.db.get())
            .await?;

        Ok(drops
            .into_iter()
            .map(|drop| (drop.id, drop.into()))
            .collect())
    }
}
