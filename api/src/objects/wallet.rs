use async_graphql::{ComplexObject, Context, Result, SimpleObject};

use crate::{entities::collection_mints, AppContext};

#[derive(SimpleObject, Debug, Clone)]
#[graphql(complex)]
pub struct Wallet {
    #[graphql(external)]
    pub address: String,
}

#[ComplexObject]
impl Wallet {
    async fn mints(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<Vec<collection_mints::CollectionMint>>> {
        let AppContext {
            collection_mints_owner_loader,
            ..
        } = ctx.data::<AppContext>()?;

        collection_mints_owner_loader
            .load_one(self.address.clone())
            .await
    }
}
