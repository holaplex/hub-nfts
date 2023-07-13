use async_graphql::{Error, InputObject, Result};
use serde::{Deserialize, Serialize};

use crate::proto;

/// An attributed creator for a collection or mint.
#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
#[graphql(name = "CreatorInput")]
pub struct Creator {
    /// The wallet address of the creator.
    pub address: String,
    /// This field indicates whether the creator has been verified. This feature is only supported on the Solana blockchain.
    /// ## References
    /// [Metaplex Token Metadata - Verify creator instruction](https://docs.metaplex.com/programs/token-metadata/instructions#verify-a-creator)
    pub verified: Option<bool>,
    /// The share of royalties payout the creator should receive.
    pub share: u8,
}

impl TryFrom<Creator> for proto::Creator {
    type Error = Error;

    fn try_from(
        Creator {
            address,
            verified,
            share,
        }: Creator,
    ) -> Result<Self> {
        Ok(Self {
            address: address.parse()?,
            verified: verified.unwrap_or_default(),
            share: share.into(),
        })
    }
}
