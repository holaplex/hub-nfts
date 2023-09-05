use std::{collections::HashSet, time::Duration};

use hub_core::{
    anyhow::Result,
    backon,
    backon::Retryable,
    futures_util::stream::FuturesUnordered,
    prelude::*,
    thiserror,
    tokio::{self, sync::mpsc},
    uuid::Uuid,
};
use metadata_jsons::Column as MetadataJsonColumn;
use sea_orm::{prelude::*, sea_query::OnConflict, QuerySelect, Set, TransactionTrait};

use crate::{
    db::Connection,
    entities::{
        metadata_json_attributes, metadata_json_files, metadata_json_jobs, metadata_jsons,
        prelude::{MetadataJsonAttributes, MetadataJsonFiles, MetadataJsons},
        sea_orm_active_enums::MetadataJsonJobType,
    },
    nft_storage::NftStorageClient,
    objects::MetadataJsonInput,
};

#[derive(Debug, thiserror::Error, Triage)]
#[fatal]
#[error("Unable to send message to metadata JSON job runner - the task has probably crashed!")]
pub struct JobRunnerError(mpsc::error::SendError<()>);

#[derive(Debug, Clone)]
pub struct JobRunner(mpsc::Sender<()>);

impl JobRunner {
    #[must_use]
    pub fn new(db: Connection, client: NftStorageClient) -> (Self, tokio::task::JoinHandle<()>) {
        let (tx, rx) = mpsc::channel(1);

        (
            JobRunner(tx),
            tokio::task::spawn(job_runner(rx, db, client)),
        )
    }

    /// Refresh the list of running metadata JSON jobs
    ///
    /// # Errors
    /// Returns a fatal error if the job runner task cannot be reached
    pub async fn refresh(&self) -> Result<(), JobRunnerError> {
        self.0.send(()).await.map_err(JobRunnerError)
    }
}

async fn job_runner(mut refresh: mpsc::Receiver<()>, db: Connection, client: NftStorageClient) {
    let mut started_jobs = HashSet::new();
    let tasks = FuturesUnordered::new();
    let backoff = backon::ExponentialBuilder::default()
        .with_jitter()
        .with_min_delay(Duration::from_millis(500))
        .with_max_times(5);

    loop {
        let jobs = (|| async {
            let res = metadata_json_jobs::Entity::find()
                .limit(16)
                .all(db.get())
                .await
                .context("Error getting metadata JSON jobs from DB");

            if let Err(e) = &res {
                error!("{e:?}");
            }

            res
        })
        .retry(&backoff)
        .await
        .unwrap_or_default();

        for job in jobs {
            let true = started_jobs.insert(job.id) else { continue };

            let db = db.clone();
            let client = client.clone();
            let backoff = backoff.clone();
            tasks.push(tokio::task::spawn(async move {
                (|| async { run_job(&db, &client, &job).await })
                    .retry(&backoff)
                    .await
            }));
        }

        let Some(()) = refresh.recv().await else { break };
    }
}

async fn run_job(
    db: &Connection,
    client: &NftStorageClient,
    job: &metadata_json_jobs::Model,
) -> Result<()> {
    match job.r#type {
        MetadataJsonJobType::Download => todo!(),
        MetadataJsonJobType::Upload => {
            MetadataJson::fetch(
                job.metadata_json_id
                    .context("Missing metadata JSON ID from upload job")?,
                db,
            )
            .await
            .context("Error fetching metadata JSON")?
            .upload(client)
            .await
            .context("Error uploading metadata JSON")?;
        },
    }

    Ok(())
}

#[derive(Clone, Debug)]
pub struct MetadataJson {
    pub metadata_json: MetadataJsonInput,
    pub uri: Option<String>,
    pub identifier: Option<String>,
}

impl MetadataJson {
    #[must_use]
    pub fn new(metadata_json: MetadataJsonInput) -> Self {
        Self {
            metadata_json,
            uri: None,
            identifier: None,
        }
    }

    /// Fetches metadata from the database and constructs a `Self` instance.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID of the metadata to fetch.
    /// * `db` - The database connection to use.
    ///
    /// # Errors
    ///
    /// This function fails if there is no matching `metadata_json` entry in the database
    /// or if it is unable to fetch related data from the database
    pub async fn fetch(id: Uuid, db: &Connection) -> Result<Self> {
        let (metadata_json_model, attributes) = metadata_jsons::Entity::find_by_id(id)
            .find_with_related(MetadataJsonAttributes)
            .all(db.get())
            .await?
            .first()
            .map(ToOwned::to_owned)
            .ok_or(anyhow!("no metadata_json entry found in db"))?;

        let files = metadata_json_files::Entity::find()
            .filter(metadata_json_files::Column::MetadataJsonId.eq(id))
            .all(db.get())
            .await?;

        let metadata_json = (metadata_json_model.clone(), attributes, Some(files)).into();

        Ok(Self {
            metadata_json,
            uri: Some(metadata_json_model.uri.clone()),
            identifier: Some(metadata_json_model.identifier),
        })
    }

    /// Res
    ///
    /// # Errors
    /// This function fails if unable to upload `metadata_json` to nft.storage
    pub async fn upload(&mut self, nft_storage: &NftStorageClient) -> Result<&Self> {
        let response = nft_storage.upload(self.metadata_json.clone()).await?;
        let cid = response.value.cid;

        let uri = nft_storage.ipfs_endpoint.join(&cid)?.to_string();

        self.uri = Some(uri);
        self.identifier = Some(cid);

        Ok(self)
    }

    /// Res
    ///
    /// # Errors
    /// This function fails if unable to save `metadata_json` to the db
    pub async fn save(&self, id: Uuid, db: &Connection) -> Result<metadata_jsons::Model> {
        let payload = self.metadata_json.clone();
        let identifier = self
            .identifier
            .clone()
            .ok_or_else(|| anyhow!("no identifier. call #upload before #save"))?;
        let uri = self
            .uri
            .clone()
            .ok_or_else(|| anyhow!("no uri. call #upload before #save"))?;

        let metadata_json_active_model = metadata_jsons::ActiveModel {
            id: Set(id),
            identifier: Set(identifier),
            name: Set(payload.name),
            uri: Set(uri),
            symbol: Set(payload.symbol),
            description: Set(payload.description),
            image: Set(payload.image),
            animation_url: Set(payload.animation_url),
            external_url: Set(payload.external_url),
        };

        let metadata_json = MetadataJsons::insert(metadata_json_active_model)
            .on_conflict(
                OnConflict::column(MetadataJsonColumn::Id)
                    .update_columns([
                        MetadataJsonColumn::Identifier,
                        MetadataJsonColumn::Name,
                        MetadataJsonColumn::Uri,
                        MetadataJsonColumn::Symbol,
                        MetadataJsonColumn::Description,
                        MetadataJsonColumn::Image,
                        MetadataJsonColumn::AnimationUrl,
                        MetadataJsonColumn::ExternalUrl,
                    ])
                    .clone(),
            )
            .exec_with_returning(db.get())
            .await?;

        let tx = db.get().clone().begin().await?;

        MetadataJsonAttributes::delete_many()
            .filter(metadata_json_attributes::Column::MetadataJsonId.eq(metadata_json.id))
            .exec(&tx)
            .await?;

        for attribute in payload.attributes {
            let am = metadata_json_attributes::ActiveModel {
                metadata_json_id: Set(metadata_json.id),
                trait_type: Set(attribute.trait_type),
                value: Set(attribute.value),
                ..Default::default()
            };

            am.insert(&tx).await?;
        }

        tx.commit().await?;

        if let Some(files) = payload.properties.unwrap_or_default().files {
            let tx = db.get().clone().begin().await?;

            MetadataJsonFiles::delete_many()
                .filter(metadata_json_files::Column::MetadataJsonId.eq(metadata_json.id))
                .exec(&tx)
                .await?;

            for file in files {
                let metadata_json_file_am = metadata_json_files::ActiveModel {
                    metadata_json_id: Set(metadata_json.id),
                    uri: Set(file.uri),
                    file_type: Set(file.file_type),
                    ..Default::default()
                };

                metadata_json_file_am.insert(&tx).await?;
            }

            tx.commit().await?;
        }

        Ok(metadata_json)
    }
}
