use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use hub_core::uuid::Uuid;
use sea_orm::FromQueryResult;

use crate::AppContext;

/// The holder of a collection.
#[derive(SimpleObject, Debug, Clone, FromQueryResult)]
#[graphql(complex)]
pub struct Holder {
    /// The collection ID that the holder owns.
    pub collection_id: Uuid,
    /// The wallet address of the holder.
    pub address: String,
    /// The number of NFTs that the holder owns in the collection.
    pub owns: i64,
}

#[ComplexObject]
impl Holder {
    /// The specific mints from the collection that the holder owns.
    async fn mints(&self, ctx: &Context<'_>) -> Result<Vec<Uuid>> {
        let AppContext {
            collection_mints_loader,
            ..
        } = ctx.data::<AppContext>()?;

        let mints = collection_mints_loader
            .load_one(self.collection_id)
            .await?
            .unwrap_or_default();

        Ok(mints.into_iter().fold(Vec::new(), |mut acc, mint| {
            if mint.owner == Some(self.address.clone()) {
                acc.push(mint.id);
            }

            acc
        }))
    }
}
