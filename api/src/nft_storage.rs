use std::collections::HashMap;

use hub_core::{
    anyhow::Result,
    backon::{ExponentialBuilder, Retryable},
    clap,
    prelude::*,
};
use reqwest::Response;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Arguments for establishing a nft storage connection
#[derive(Debug, clap::Args)]
pub struct NftStorageArgs {
    #[arg(long, env)]
    pub nft_storage_api_endpoint: String,
    #[arg(long, env)]
    pub nft_storage_auth_token: String,
    #[arg(long, env)]
    pub ipfs_endpoint: String,
}

#[derive(Debug, Clone)]
pub struct NftStorageClient {
    http: reqwest::Client,
    auth: String,
    pub api_base_url: Url,
    pub ipfs_endpoint: Url,
}

impl NftStorageClient {
    /// Returns the `NftStorage` client
    ///
    /// # Errors
    /// if http client fails to build or url parsing fails
    pub fn new(args: NftStorageArgs) -> Result<Self> {
        let NftStorageArgs {
            nft_storage_api_endpoint,
            nft_storage_auth_token,
            ipfs_endpoint,
        } = args;

        let api_base_url = Url::parse(&nft_storage_api_endpoint)
            .context("failed to parse nft storage base url")?;
        let ipfs_endpoint = Url::parse(&ipfs_endpoint)?;

        Ok(Self {
            http: reqwest::Client::new(),
            auth: nft_storage_auth_token,
            api_base_url,
            ipfs_endpoint,
        })
    }

    /// Returns the response of the post request to nft.storage api
    ///
    /// # Errors
    /// Post request can fail if the auth token/payload is invalid or the api is down
    pub async fn post(&self, endpoint: String, body: impl Serialize) -> Result<Response> {
        let url = self.api_base_url.join(&endpoint)?;

        self.http
            .post(url)
            .bearer_auth(&self.auth)
            .json(&body)
            .send()
            .await
            .context("failed to send post request")
    }

    /// Uploads the json data and returns the response
    ///
    /// # Errors
    /// If the upload fails
    pub async fn upload(&self, data: &impl Serialize) -> Result<UploadResponse> {
        let post = || self.post("/upload".to_string(), data);

        post.retry(
            &ExponentialBuilder::default()
                .with_jitter()
                .with_min_delay(Duration::from_millis(30))
                .with_max_times(15),
        )
        .await?
        .json()
        .await
        .context("failed to parse response")
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UploadResponse {
    pub ok: bool,
    pub value: Nft,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Nft {
    pub cid: String,
    pub size: u64,
    pub created: String,
    pub r#type: String,
    #[serde(flatten)]
    extra: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]

pub struct Pin {
    pub cid: String,
    pub name: String,
    pub status: String,
    pub created: String,
    pub size: String,
    #[serde(flatten)]
    extra: HashMap<String, Value>,
}
