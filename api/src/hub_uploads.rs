use hub_core::{
    anyhow::Result,
    backon::{ExponentialBuilder, Retryable},
    clap,
    prelude::*,
};
use reqwest::{
    multipart::{Form, Part},
    Response,
};
use serde::{Deserialize, Serialize};

/// Arguments for establishing a nft storage connection
#[derive(Debug, clap::Args)]
pub struct HubUploadArgs {
    #[arg(long, env)]
    pub hub_uploads_api_endpoint: String,
}

#[derive(Debug, Clone)]
pub struct HubUploadClient {
    http: reqwest::Client,
    pub api_base_url: Url,
}

impl HubUploadClient {
    /// Returns the `NftStorage` client
    ///
    /// # Errors
    /// if http client fails to build or url parsing fails
    pub fn new(args: HubUploadArgs) -> Result<Self> {
        let HubUploadArgs {
            hub_uploads_api_endpoint,
        } = args;

        let api_base_url = Url::parse(&hub_uploads_api_endpoint)
            .context("failed to parse nft storage base url")?;

        Ok(Self {
            http: reqwest::Client::new(),
            api_base_url,
        })
    }

    /// Returns the response of the post request to nft.storage api
    ///
    /// # Errors
    /// Post request can fail if the auth token/payload is invalid or the api is down
    async fn post(&self, endpoint: String, body: impl Serialize) -> Result<Response> {
        let url = self.api_base_url.join(&endpoint)?;

        let serialized_body = serde_json::to_vec(&body).context("failed to serialize body")?;
        let part = Part::bytes(serialized_body).file_name("file_name.extension");

        let form = Form::new().part("file", part);

        self.http
            .post(url)
            .multipart(form)
            .send()
            .await
            .context("failed to send post request")
    }

    /// Uploads the json data and returns the response
    ///
    /// # Errors
    /// If the upload fails
    pub async fn upload(&self, data: &impl Serialize) -> Result<UploadResponse> {
        let post = || self.post("/uploads".to_string(), data);

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
    pub uri: String,
    pub cid: String,
}
