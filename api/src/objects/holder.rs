use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use hub_core::uuid::Uuid;
use sea_orm::FromQueryResult;

use crate::AppContext;

#[derive(SimpleObject, Debug, Clone, FromQueryResult)]
#[graphql(complex)]
pub struct Holder {
    pub collection_id: Uuid,
    pub address: String,
    pub owns: i64,
}

#[ComplexObject]
impl Holder {
    async fn mints(&self, ctx: &Context<'_>) -> Result<Vec<String>> {
        let AppContext {
            collection_mints_loader,
            ..
        } = ctx.data::<AppContext>()?;

        let mints = collection_mints_loader
            .load_one(self.collection_id)
            .await?
            .unwrap_or_default();

        Ok(mints.into_iter().fold(Vec::new(), |mut acc, mint| {
            if mint.owner == self.address {
                acc.push(mint.address);
            }

            acc
        }))
    }
}
