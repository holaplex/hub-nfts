use std::{collections::HashSet, time::Duration};

use hub_core::{
    anyhow::Result,
    backon,
    backon::Retryable,
    futures_util::stream::FuturesUnordered,
    prelude::*,
    producer::Producer,
    thiserror,
    tokio::{self, sync::mpsc},
    uuid::Uuid,
};
use metadata_jsons::Column as MetadataJsonColumn;
use sea_orm::{prelude::*, sea_query::OnConflict, QuerySelect, Set, TransactionTrait};

use crate::{
    blockchains::{polygon::Polygon, solana::Solana},
    db::Connection,
    entities::{
        metadata_json_attributes, metadata_json_files, metadata_json_jobs, metadata_json_uploads,
        metadata_jsons,
        prelude::{MetadataJsonAttributes, MetadataJsonFiles, MetadataJsons},
        sea_orm_active_enums::MetadataJsonJobType,
    },
    mutations::{
        collection::{
            finish_create_collection, finish_patch_collection, FinishCreateCollectionArgs,
            FinishPatchCollectionArgs,
        },
        drop::{finish_create_drop, finish_patch_drop, FinishCreateDropArgs, FinishPatchDropArgs},
        mint::{
            finish_mint_to_collection, finish_update_mint, FinishMintToCollectionArgs,
            FinishUpdateMintArgs,
        },
    },
    nft_storage::NftStorageClient,
    objects::MetadataJsonInput,
    proto::NftEvents,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Continuation {
    CreateCollection(FinishCreateCollectionArgs),
    PatchCollection(FinishPatchCollectionArgs),
    CreateDrop(FinishCreateDropArgs),
    PatchDrop(FinishPatchDropArgs),
    MintToCollection(FinishMintToCollectionArgs),
    UpdateMint(FinishUpdateMintArgs),
}

type JobRunnerMessage = metadata_json_jobs::ActiveModel;
pub type JobResult = Result<()>;

#[derive(Debug, thiserror::Error, Triage)]
#[fatal]
#[error("Unable to send message to metadata JSON job runner - the task has probably crashed!")]
pub struct JobRunnerError(mpsc::error::SendError<JobRunnerMessage>);

#[derive(Debug, Clone)]
pub struct JobRunner(mpsc::Sender<JobRunnerMessage>);

impl JobRunner {
    #[must_use]
    pub fn new(
        ctx: JobRunnerContext,
        client: NftStorageClient,
    ) -> (Self, tokio::task::JoinHandle<()>) {
        let (tx, rx) = mpsc::channel(1);

        (
            JobRunner(tx),
            tokio::task::spawn(job_runner(rx, ctx, client)),
        )
    }

    /// Submit a new metadata JSON job
    ///
    /// # Errors
    /// Returns a fatal error if the job runner task cannot be reached
    pub async fn submit(&self, job: JobRunnerMessage) -> Result<(), JobRunnerError> {
        self.0.send(job).await.map_err(JobRunnerError)
    }
}

#[derive(Clone)]
pub struct JobRunnerContext {
    pub db: Connection,
    pub solana: Solana,
    pub polygon: Polygon,
    pub nfts_producer: Producer<NftEvents>,
}

pub struct JobContext<'a> {
    runner_ctx: &'a JobRunnerContext,
    pub metadata_json: metadata_jsons::Model,
    pub upload: metadata_json_uploads::Model,
}

impl std::ops::Deref for JobContext<'_> {
    type Target = JobRunnerContext;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.runner_ctx
    }
}

async fn job_runner(
    mut rx: mpsc::Receiver<JobRunnerMessage>,
    ctx: JobRunnerContext,
    client: NftStorageClient,
) {
    let mut started_jobs = HashSet::new();
    let mut tasks = FuturesUnordered::new();
    let backoff = backon::ExponentialBuilder::default()
        .with_jitter()
        .with_min_delay(Duration::from_millis(500))
        .with_max_times(5);
    let failed = metadata_json_jobs::ActiveModel {
        failed: Set(true),
        ..Default::default()
    };

    loop {
        enum Event {
            TaskFinished(Uuid, Result<Result<(), Error>, tokio::task::JoinError>),
            Message(Option<JobRunnerMessage>),
        }

        let jobs = (|| async {
            let res = metadata_json_jobs::Entity::find()
                .filter(metadata_json_jobs::Column::Failed.eq(false))
                .limit(16)
                .all(ctx.db.get())
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
            let id = job.id;
            let true = started_jobs.insert(id) else {
                continue;
            };

            let ctx = ctx.clone();
            let client = client.clone();
            let backoff = backoff.clone();
            tasks.push(
                tokio::task::spawn(async move {
                    (|| async { run_job(&ctx, &client, &job).await })
                        .retry(&backoff)
                        .await
                })
                .map(move |r| (r, id)),
            );
        }

        let evt = tokio::select! {
            Some((t, i)) = tasks.next() => Event::TaskFinished(i, t),
            m = rx.recv() => Event::Message(m),
        };

        match evt {
            Event::TaskFinished(id, Err(e)) => {
                error!("{:?}", Error::new(e).context("Metadata JSON job panicked"));
                metadata_json_jobs::Entity::update(failed.clone())
                    .filter(metadata_json_jobs::Column::Id.eq(id))
                    .exec(ctx.db.get())
                    .await
                    .map(|_| ())
                    .with_context(|| {
                        format!("Error marking panicked metadata JSON job {id:?} as failed")
                    })
            },
            Event::TaskFinished(id, Ok(Err(e))) => {
                error!("{:?}", e.context("Error processing metadata JSON job"));
                metadata_json_jobs::Entity::update(failed.clone())
                    .filter(metadata_json_jobs::Column::Id.eq(id))
                    .exec(ctx.db.get())
                    .await
                    .map(|_| ())
                    .with_context(|| format!("Error marking metadata JSON job {id:?} as failed"))
            },
            Event::TaskFinished(id, Ok(Ok(()))) => metadata_json_jobs::Entity::delete_by_id(id)
                .exec(ctx.db.get())
                .await
                .map(|_| ())
                .with_context(|| format!("Error dequeuing finished metadata JSON job {id:?}")),
            Event::Message(None) => break,
            Event::Message(Some(m)) => metadata_json_jobs::Entity::insert(m.clone())
                .exec(ctx.db.get())
                .await
                .map(|_| ())
                .with_context(|| format!("Error queuing metadata JSON job {m:?}")),
        }
        .unwrap_or_else(|e| panic!("{e:?}"));
    }
}

async fn run_job(
    ctx: &JobRunnerContext,
    client: &NftStorageClient,
    job: &metadata_json_jobs::Model,
) -> Result<()> {
    let metadata_json_jobs::Model {
        id: _,
        r#type: ty,
        continuation,
        failed,
        url,
        metadata_json_id,
    } = job;

    assert!(!failed);

    let (metadata_json, upload) = match ty {
        MetadataJsonJobType::Download => todo!("download {url:?}"),
        MetadataJsonJobType::Upload => {
            let metadata_json_id =
                metadata_json_id.context("Missing metadata JSON ID from upload job")?;

            let mut metadata_json = MetadataJson::fetch(metadata_json_id, &ctx.db)
                .await
                .context("Error fetching metadata JSON")?;
            let upload = metadata_json
                .upload_internal(metadata_json_id, &ctx.db, client)
                .await
                .context("Error uploading metadata JSON")?;

            (
                upload
                    .find_related(metadata_jsons::Entity)
                    .one(ctx.db.get())
                    .await?
                    .context("Missing metadata JSON associated with upload")?,
                upload.clone(),
            )
        },
    };

    if let Some(cont) = continuation {
        let ctx = JobContext {
            runner_ctx: ctx,
            metadata_json,
            upload,
        };

        let res: JobResult = match ciborium::from_reader(&mut cont.as_slice())
            .context("Error deserializing metadata JSON job continuation")?
        {
            Continuation::CreateCollection(args) => finish_create_collection(&ctx, args).await,
            Continuation::PatchCollection(args) => finish_patch_collection(&ctx, args).await,
            Continuation::CreateDrop(args) => finish_create_drop(&ctx, args).await,
            Continuation::PatchDrop(args) => finish_patch_drop(&ctx, args).await,
            Continuation::MintToCollection(args) => finish_mint_to_collection(&ctx, args).await,
            Continuation::UpdateMint(args) => finish_update_mint(&ctx, args).await,
        };

        res?;
    }

    Ok(())
}

pub trait MetadataJsonUploadInfo<'a> {
    fn into_info(self) -> Option<(&'a JobRunner, Option<Continuation>)>;
}

impl<'a> MetadataJsonUploadInfo<'a> for Option<&'a JobRunner> {
    fn into_info(self) -> Option<(&'a JobRunner, Option<Continuation>)> {
        self.map(|r| (r, None))
    }
}

impl<'a> MetadataJsonUploadInfo<'a> for (&'a JobRunner, Continuation) {
    fn into_info(self) -> Option<(&'a JobRunner, Option<Continuation>)> {
        let (r, c) = self;
        Some((r, Some(c)))
    }
}

#[derive(Clone, Debug)]
pub struct MetadataJson {
    pub metadata_json: MetadataJsonInput,
    pub upload: Option<metadata_json_uploads::Model>,
}

impl MetadataJson {
    #[must_use]
    pub fn new(metadata_json: MetadataJsonInput) -> Self {
        Self {
            metadata_json,
            upload: None,
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

        let upload = metadata_json_uploads::Entity::find()
            .filter(metadata_json_uploads::Column::Id.eq(id))
            .one(db.get())
            .await?;

        Ok(Self {
            metadata_json,
            upload,
        })
    }

    /// Res
    ///
    /// # Errors
    /// This function fails if unable to upload `metadata_json` to nft.storage
    async fn upload_internal(
        &mut self,
        id: Uuid,
        db: &Connection,
        nft_storage: &NftStorageClient,
    ) -> Result<&metadata_json_uploads::Model> {
        if self.upload.is_some() {
            bail!("Error uploading already-uploaded metadata JSON");
        }

        let response = nft_storage.upload(self.metadata_json.clone()).await?;
        let cid = response.value.cid;

        let uri = nft_storage.ipfs_endpoint.join(&cid)?.to_string();

        let upload = metadata_json_uploads::ActiveModel {
            id: Set(id),
            uri: Set(uri.clone()),
            identifier: Set(cid.clone()),
        }
        .insert(db.get())
        .await?;

        self.upload = Some(upload);

        Ok(self.upload.as_ref().unwrap_or_else(|| unreachable!()))
    }

    /// Res
    ///
    /// # Errors
    /// This function fails if unable to save `metadata_json` to the db
    pub async fn save<'a, I: MetadataJsonUploadInfo<'a> + 'a>(
        &'a self,
        id: Uuid,
        db: &'a Connection,
        upload_with: I,
    ) -> Result<metadata_jsons::Model> {
        let payload = self.metadata_json.clone();

        let metadata_json_active_model = metadata_jsons::ActiveModel {
            id: Set(id),
            name: Set(payload.name),
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
                        MetadataJsonColumn::Name,
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

        if let Some((runner, cont)) = upload_with.into_info() {
            let continuation = cont
                .map(|c| {
                    let mut vec = vec![];
                    ciborium::into_writer(&c, &mut vec)
                        .context("Error serializing metadata JSON job continuation")?;
                    Result::<_>::Ok(vec)
                })
                .transpose()?;

            runner
                .submit(metadata_json_jobs::ActiveModel {
                    r#type: Set(MetadataJsonJobType::Upload),
                    continuation: Set(continuation),
                    metadata_json_id: Set(Some(id)),
                    ..Default::default()
                })
                .await?;
        }

        Ok(metadata_json)
    }
}
