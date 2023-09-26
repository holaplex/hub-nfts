use hub_core::{anyhow::Result, prelude::*, producer::SendError, thiserror, url};
use sea_orm::error::DbErr;
use serde_json::Value as Json;

use crate::db::Connection;

mod metadata_json_upload_task;

#[derive(thiserror::Error, Debug)]
pub enum BackgroundTaskError {
    #[error("Uri string parse error: {0}")]
    UrlParse(#[from] url::ParseError),
    #[error("Hub core error: {0}")]
    HubCore(#[from] Error),
    #[error("Database error: {0}")]
    Database(#[from] DbErr),
    #[error("Blockchain not supported")]
    BlockchainNotSupported,
    #[error("Db record not found")]
    RecordNotFound,
    #[error("No supply")]
    NoSupply,
    #[error("No owner")]
    NoOwner,
    #[error("No project wallet")]
    NoProjectWallet,
    #[error("Unable to send nft event")]
    ProducerSend(#[from] SendError),
    #[error("Unable to convert value: {0}")]
    Conversion(#[from] std::convert::Infallible),
    #[error("No creator")]
    NoCreator,
}

#[async_trait::async_trait]
pub trait BackgroundTask<C: Clone>: Send + Sync + std::fmt::Debug {
    /// Process the task
    /// # Arguments
    /// * `self` - The task
    /// * `db` - The database connection
    /// * `context` - The context
    /// # Returns
    /// * `Result<(), BackgroundTaskError>` - The result of the operation
    /// # Error
    /// * `BackgroundTaskError` - The error that occurred
    async fn process(&self, db: Connection, context: C) -> Result<(), BackgroundTaskError>;
    /// Get the payload of the task
    /// # Arguments
    /// * `self` - The task
    /// # Returns
    /// * `Result<Json>` - The payload of the task
    /// # Error
    /// * `anyhow::Error` - Unable to serialize the payload
    fn payload(&self) -> Result<Json>;
    fn name(&self) -> &'static str;
}

pub use metadata_json_upload_task::{
    Caller as MetadataJsonUploadCaller, Context as MetadataJsonUploadContext,
    CreateCollection as MetadataJsonUploadCreateCollection,
    CreateDrop as MetadataJsonUploadCreateDrop, MetadataJsonUploadTask,
    MintToCollection as MetadataJsonUploadMintToCollection,
    PatchCollection as MetadataJsonUploadPatchCollection, PatchDrop as MetadataJsonUploadPatchDrop,
    QueueMintToDrop as MetadataJsonUploadQueueMintToDrop,
    UpdateMint as MetadataJsonUploadUpdateMint,
};
