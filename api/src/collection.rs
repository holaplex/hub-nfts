use hub_core::anyhow::Result;
use sea_orm::{prelude::*, Set};

use crate::{
    db::Connection,
    entities::{
        collection_creators::ActiveModel as CollectionCreatorActiveModel,
        collections::{ActiveModel, Model},
    },
    objects::CollectionCreator,
};

#[derive(Debug, Clone)]
pub struct Collection {
    collection: ActiveModel,
    creators: Option<Vec<CollectionCreator>>,
}

impl Collection {
    #[must_use]
    pub fn new(collection: ActiveModel) -> Self {
        Self {
            collection,
            creators: None,
        }
    }

    pub fn creators(&mut self, creators: Vec<CollectionCreator>) -> &Collection {
        self.creators = Some(creators);

        self
    }

    /// Res
    ///
    /// # Errors
    /// This function fails if unable to save `collection` or `collection_creators` to the db
    pub async fn save(&self, db: &Connection) -> Result<Model> {
        let conn = db.get();

        let collection = self.collection.clone().insert(conn).await?;

        let creators = self.creators.clone().unwrap_or_default();

        for creator in creators {
            let am = CollectionCreatorActiveModel {
                collection_id: Set(collection.id),
                address: Set(creator.address),
                verified: Set(creator.verified.unwrap_or_default()),
                share: Set(creator.share.try_into()?),
            };

            am.insert(conn).await?;
        }

        Ok(collection)
    }
}
