use async_graphql::{Error, InputObject, Result};
use serde::{Deserialize, Serialize};

use crate::proto;

/// An attributed creator for a colleciton.
#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
#[graphql(name = "CollectionCreatorInput")]
pub struct CollectionCreator {
    /// The wallet address of the creator.
    pub address: String,
    /// This field indicates whether the collection's creator has been verified. This feature is only supported on the Solana blockchain.
    /// ## References
    /// [Metaplex Token Metadata - Verify creator instruction](https://docs.metaplex.com/programs/token-metadata/instructions#verify-a-creator)
    pub verified: Option<bool>,
    /// The share of royalties payout the creator should receive.
    pub share: u8,
}

impl TryFrom<CollectionCreator> for proto::Creator {
    type Error = Error;

    fn try_from(
        CollectionCreator {
            address,
            verified,
            share,
        }: CollectionCreator,
    ) -> Result<Self> {
        Ok(Self {
            address: address.parse()?,
            verified: verified.unwrap_or_default(),
            share: share.into(),
        })
    }
}
