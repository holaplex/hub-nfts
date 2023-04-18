use std::collections::HashMap;

use async_graphql::{dataloader::Loader as DataLoader, FieldError, Result};
use poem::async_trait;
use sea_orm::prelude::*;

use crate::{
    db::Connection,
    entities::{metadata_json_attributes, metadata_jsons},
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
    type Value = metadata_jsons::Model;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let metadata_jsons = metadata_jsons::Entity::find()
            .filter(metadata_jsons::Column::Id.is_in(keys.iter().map(ToOwned::to_owned)))
            .all(self.db.get())
            .await?;

        Ok(metadata_jsons
            .into_iter()
            .map(|metadata_json| (metadata_json.id, metadata_json))
            .collect())
    }
}

#[derive(Debug, Clone)]
pub struct AttributesLoader {
    pub db: Connection,
}

impl AttributesLoader {
    #[must_use]
    pub fn new(db: Connection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DataLoader<Uuid> for AttributesLoader {
    type Error = FieldError;
    type Value = Vec<metadata_json_attributes::Model>;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let metadata_json_attributes = metadata_json_attributes::Entity::find()
            .filter(
                metadata_json_attributes::Column::MetadataJsonId
                    .is_in(keys.iter().map(ToOwned::to_owned)),
            )
            .all(self.db.get())
            .await?;

        Ok(metadata_json_attributes.into_iter().fold(
            HashMap::new(),
            |mut acc, metadata_json_attribute| {
                acc.entry(metadata_json_attribute.metadata_json_id)
                    .or_insert_with(Vec::new);

                acc.entry(metadata_json_attribute.metadata_json_id)
                    .and_modify(|attributes| attributes.push(metadata_json_attribute));

                acc
            },
        ))
    }
}
