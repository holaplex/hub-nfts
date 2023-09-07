pub mod polygon;
pub mod solana;

use hub_core::anyhow::Result;

use crate::proto::{
    NftEventKey, RetryUpdateSolanaMintPayload, SwitchCollectionPayload, UpdateSolanaMintPayload,
};

/// Represents a response from a transaction on the blockchain. This struct
/// provides the serialized message and the signatures of the signed message.
pub struct TransactionResponse {
    /// The serialized version of the message from the transaction.
    pub serialized_message: Vec<u8>,

    /// The signatures of the signed message from the transaction.
    pub signed_message_signatures: Vec<String>,
}

#[async_trait::async_trait]
pub trait DropEvent<A, B, C> {
    async fn create_drop(&self, key: NftEventKey, payload: A) -> Result<()>;
    async fn retry_create_drop(&self, key: NftEventKey, payload: A) -> Result<()>;
    async fn update_drop(&self, key: NftEventKey, payload: C) -> Result<()>;
    async fn mint_drop(&self, key: NftEventKey, payload: B) -> Result<()>;
    async fn retry_mint_drop(&self, key: NftEventKey, payload: B) -> Result<()>;
}

#[async_trait::async_trait]
pub trait CollectionEvent<A, B, C> {
    async fn create_collection(&self, key: NftEventKey, payload: A) -> Result<()>;
    async fn retry_create_collection(&self, key: NftEventKey, payload: A) -> Result<()>;
    async fn update_collection(&self, key: NftEventKey, payload: B) -> Result<()>;
    async fn mint_to_collection(&self, key: NftEventKey, payload: C) -> Result<()>;
    async fn retry_mint_to_collection(&self, key: NftEventKey, payload: C) -> Result<()>;
    async fn update_collection_mint(
        &self,
        key: NftEventKey,
        payload: UpdateSolanaMintPayload,
    ) -> Result<()>;
    async fn retry_update_mint(
        &self,
        key: NftEventKey,
        payload: RetryUpdateSolanaMintPayload,
    ) -> Result<()>;
    async fn switch_collection(
        &self,
        key: NftEventKey,
        payload: SwitchCollectionPayload,
    ) -> Result<()>;
}

#[async_trait::async_trait]
pub trait TransferEvent<A> {
    async fn transfer_asset(&self, key: NftEventKey, payload: A) -> Result<()>;
}
