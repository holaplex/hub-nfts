pub mod solana;

use hub_core::anyhow::Result;
use solana_program::pubkey::Pubkey;
pub struct TransactionResponse {
    pub serialized_message: Vec<u8>,
    pub signed_message_signatures: Vec<String>,
}

pub trait Blockchain<D, E> {
    async fn drop(&self, payload: D) -> Result<TransactionResponse>;
    async fn edition(&self, payload: E) -> Result<(Pubkey, TransactionResponse)>;
}
