use hub_core::{anyhow::Result, uuid::Uuid};

pub struct Transaction {
    pub serialized_message: Vec<u8>,
    pub signed_message_signatures: Vec<String>,
}

pub trait Blockchain<D, E> {
    async fn drop(&self, input: D, collection_id: Uuid) -> Result<Transaction>;
    async fn edition(&self, input: E) -> Result<Transaction>;
}
