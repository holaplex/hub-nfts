use std::collections::HashMap;

use async_graphql::{dataloader::Loader as DataLoader, FieldError, Result};
use poem::async_trait;
use sea_orm::{prelude::*, JoinType, QuerySelect};

use crate::{
    db::Connection,
    entities::{collections, drops},
    objects::Drop,
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
    type Value = Drop;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let drops = drops::Entity::find()
            .join(JoinType::InnerJoin, drops::Relation::Collections.def())
            .select_also(collections::Entity)
            .filter(drops::Column::Id.is_in(keys.iter().map(ToOwned::to_owned)))
            .all(self.db.get())
            .await?;

        drops
            .into_iter()
            .map(|(drop, collection)| {
                Ok((
                    drop.id,
                    Drop::new(
                        drop.clone(),
                        collection.ok_or_else(|| {
                            FieldError::new(format!("no collection for the drop {}", drop.id))
                        })?,
                    ),
                ))
            })
            .collect::<Result<HashMap<Uuid, Self::Value>>>()
    }
}
