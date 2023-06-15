use async_graphql::{ComplexObject, Context, Result, SimpleObject};

use crate::{entities::collection_mints, AppContext};

/// A blockchain wallet is a digital wallet that allows users to securely store, manage, and transfer their cryptocurrencies or other digital assets on a blockchain network.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(complex)]
pub struct Wallet {
    /// A blockchain wallet address is a unique identifier that represents a destination for transactions on a blockchain network. It is a string of alphanumeric characters that can be used to receive and send digital assets, such as cryptocurrencies, on the blockchain network.
    #[graphql(external)]
    pub address: Option<String>,
}

#[ComplexObject]
impl Wallet {
    /// The NFTs that were minted from Holaplex and are owned by the wallet's address.
    async fn mints(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<Vec<collection_mints::CollectionMint>>> {
        let AppContext {
            collection_mints_owner_loader,
            ..
        } = ctx.data::<AppContext>()?;

        collection_mints_owner_loader
            .load_one(self.address.clone().unwrap_or(String::new()))
            .await
    }
}
