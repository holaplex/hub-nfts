pub mod solana;

use hub_core::anyhow::Result;

pub struct TransactionResponse {
    pub serialized_message: Vec<u8>,
    pub signed_message_signatures: Vec<String>,
}

pub trait Blockchain<D, E, M> {
    async fn drop(&self, payload: D) -> Result<(M, TransactionResponse)>;
    async fn edition(&self, payload: E) -> Result<(M, TransactionResponse)>;
}
