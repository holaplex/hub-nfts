use async_graphql::{ComplexObject, Context, Result, SimpleObject};
use hub_core::uuid::Uuid;

use crate::{entities::collection_mints, AppContext};

/// A project customer.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(complex)]
pub struct Customer {
    /// The unique identifier of the customer
    pub id: Uuid,
    #[graphql(external)]
    /// The treasury assigned to the customer
    pub addresses: Option<Vec<String>>,
}

#[ComplexObject]
impl Customer {
    /// The NFTs owned by any of the customers' wallets.
    #[graphql(requires = "addresses")]
    async fn mints(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<Vec<collection_mints::CollectionMint>>> {
        let AppContext {
            collection_mints_owner_loader,
            ..
        } = ctx.data::<AppContext>()?;

        if let Some(addresses) = self.addresses.clone() {
            Ok(Some(
                collection_mints_owner_loader
                    .load_many(addresses)
                    .await?
                    .into_values()
                    .flatten()
                    .collect(),
            ))
        } else {
            Ok(None)
        }
    }
}
