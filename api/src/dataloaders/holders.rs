use std::collections::HashMap;

use async_graphql::{dataloader::Loader as DataLoader, FieldError, Result};
use poem::async_trait;
use sea_orm::{prelude::*, QuerySelect};

use crate::{db::Connection, entities::collection_mints, objects::Holder};

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
    type Value = Vec<Holder>;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let holders = collection_mints::Entity::find()
            .filter(
                collection_mints::Column::CollectionId.is_in(keys.iter().map(ToOwned::to_owned)),
            )
            .select_only()
            .column(collection_mints::Column::CollectionId)
            .column_as(collection_mints::Column::Owner, "address")
            .column_as(collection_mints::Column::Id.count(), "owns")
            .group_by(collection_mints::Column::Owner)
            .group_by(collection_mints::Column::CollectionId)
            .into_model::<Holder>()
            .all(self.db.get())
            .await?;

        Ok(holders.into_iter().fold(HashMap::new(), |mut acc, holder| {
            acc.entry(holder.collection_id).or_insert_with(Vec::new);

            acc.entry(holder.collection_id)
                .and_modify(|holders| holders.push(holder));

            acc
        }))
    }
}
