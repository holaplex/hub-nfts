use std::collections::HashMap;

use async_graphql::{dataloader::Loader as DataLoader, FieldError, Result};
use poem::async_trait;
use sea_orm::{prelude::*, JoinType, QuerySelect};

use crate::{
    db::Connection,
    entities::{collection_creators, collections},
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
    type Value = Vec<collection_creators::Model>;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let collections = collections::Entity::find()
            .join(
                JoinType::InnerJoin,
                collections::Relation::CollectionCreators.def(),
            )
            .select_with(collection_creators::Entity)
            .filter(collections::Column::Id.is_in(keys.iter().map(ToOwned::to_owned)))
            .all(self.db.get())
            .await?;

        Ok(collections
            .into_iter()
            .map(|(collection, creators)| (collection.id, creators))
            .collect())
    }
}
