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
            .join(JoinType::InnerJoin, drops::Relation::Collections.def())
            .select_also(collections::Entity)
            .filter(drops::Column::ProjectId.is_in(keys.iter().map(ToOwned::to_owned)))
            .all(self.db.get())
            .await?;

        Ok(drops
            .into_iter()
            .filter_map(|(drop, collection)| {
                collection.map(|collection| (drop.project_id, Drop::new(drop, collection)))
            })
            .fold(HashMap::new(), |mut acc, (project, drop)| {
                acc.entry(project).or_insert_with(Vec::new);

                acc.entry(project).and_modify(|drops| drops.push(drop));

                acc
            }))
    }
}
