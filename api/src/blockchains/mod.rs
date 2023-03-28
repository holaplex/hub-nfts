pub mod solana;

use hub_core::anyhow::Result;

pub struct TransactionResponse {
    pub serialized_message: Vec<u8>,
    pub signed_message_signatures: Vec<String>,
}

#[async_trait::async_trait]

pub trait Edition<A, B, C, M> {
    async fn create(&self, payload: A) -> Result<(M, TransactionResponse)>;
    async fn mint(&self, payload: B) -> Result<(M, TransactionResponse)>;
    async fn update(&self, payload: C) -> Result<(M, TransactionResponse)>;
}
