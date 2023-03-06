use async_graphql::{Context, Object, Result};

use crate::objects::Wallet;

#[derive(Debug, Clone, Copy, Default)]
pub struct Query;

#[Object(name = "WalletQuery")]
impl Query {
    /// Res
    ///
    /// # Errors
    /// This function fails if unable to set the project
    #[graphql(entity)]
    async fn find_wallet_by_address(
        &self,
        _ctx: &Context<'_>,
        #[graphql(key)] address: String,
    ) -> Result<Wallet> {
        Ok(Wallet { address })
    }
}
