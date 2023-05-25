pub mod solana;

use hub_core::{anyhow::Result, uuid::Uuid};

/// Represents a response from a transaction on the blockchain. This struct
/// provides the serialized message and the signatures of the signed message.
pub struct TransactionResponse {
    /// The serialized version of the message from the transaction.
    pub serialized_message: Vec<u8>,

    /// The signatures of the signed message from the transaction.
    pub signed_message_signatures: Vec<String>,
}

/// A trait that defines the fundamental operations that can be performed
/// on a given blockchain for a specific edition of an NFT.
#[async_trait::async_trait]
pub trait Edition<A, B, C, D, E, M> {
    /// Creates a new NFT on the blockchain. The specifics of the creation
    /// process, such as the parameters it takes and the values it returns,
    /// are dependent on the implementation of this method for the specific blockchain.
    async fn create(&self, payload: A) -> Result<(M, TransactionResponse)>;

    /// Mints a new instance of the NFT on the blockchain. The specifics of the minting
    /// process, such as the parameters it takes and the values it returns,
    /// are dependent on the implementation of this method for the specific blockchain.
    async fn mint(&self, payload: B) -> Result<(M, TransactionResponse)>;

    /// Updates an existing collection on the blockchain. The specifics of the update
    /// process, such as the parameters it takes and the values it returns,
    /// are dependent on the implementation of this method for the specific blockchain.
    async fn update(&self, payload: C) -> Result<(M, TransactionResponse)>;

    /// Transfers an NFT from one account to another on the blockchain. The specifics of the transfer
    /// process, such as the parameters it takes and the values it returns,
    /// are dependent on the implementation of this method for the specific blockchain.
    async fn transfer(&self, payload: D) -> Result<(Uuid, TransactionResponse)>;

    /// Retries a failed drop of an NFT on the blockchain. The specifics of the retry drop
    /// process, such as the parameters it takes and the values it returns,
    /// are dependent on the implementation of this method for the specific blockchain.
    async fn retry_drop(&self, payload: E) -> Result<(M, TransactionResponse)>;
}
