use std::collections::HashMap;

use async_graphql::{dataloader::Loader as DataLoader, FieldError, Result};
use poem::async_trait;
use sea_orm::prelude::*;

use crate::{db::Connection, entities::mint_creators};

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
    type Value = Vec<mint_creators::Model>;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let mint_creators = mint_creators::Entity::find()
            .filter(
                mint_creators::Column::CollectionMintId.is_in(keys.iter().map(ToOwned::to_owned)),
            )
            .all(self.db.get())
            .await?;

        Ok(mint_creators
            .into_iter()
            .fold(HashMap::new(), |mut acc, mint_creator| {
                acc.entry(mint_creator.collection_mint_id)
                    .or_insert_with(Vec::new);

                acc.entry(mint_creator.collection_mint_id)
                    .and_modify(|mint_creators| mint_creators.push(mint_creator));

                acc
            }))
    }
}
