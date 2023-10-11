use std::collections::HashMap;

use async_graphql::{dataloader::Loader as DataLoader, FieldError, Result};
use poem::async_trait;
use sea_orm::prelude::*;

use crate::{db::Connection, entities::drops, objects::Drop};

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
    type Value = Vec<Drop>;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let drops = drops::Entity::find()
            .filter(drops::Column::ProjectId.is_in(keys.iter().map(ToOwned::to_owned)))
            .all(self.db.get())
            .await?;

        Ok(drops
            .into_iter()
            .map(|drop| (drop.project_id, drop.into()))
            .fold(HashMap::new(), |mut acc, (project, drop)| {
                acc.entry(project).or_insert_with(Vec::new);

                acc.entry(project).and_modify(|drops| drops.push(drop));

                acc
            }))
    }
}
