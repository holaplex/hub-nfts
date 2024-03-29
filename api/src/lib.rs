#![deny(clippy::disallowed_methods, clippy::suspicious, clippy::style)]
#![warn(clippy::pedantic, clippy::cargo)]
#![allow(clippy::module_name_repetitions)]

pub mod background_worker;
pub mod blockchains;
pub mod dataloaders;
pub mod db;
pub mod entities;
pub mod events;
pub mod handlers;
pub mod hub_uploads;
pub mod metrics;
pub mod mutations;
pub mod objects;
pub mod queries;

use async_graphql::{
    dataloader::DataLoader,
    extensions::{ApolloTracing, Logger},
    EmptySubscription, Schema,
};
use background_worker::job_queue::JobQueue;
use blockchains::{polygon::Polygon, solana::Solana};
use dataloaders::{
    CollectionDropLoader, CollectionLoader, CollectionMintHistoriesLoader, CollectionMintLoader,
    CollectionMintMintHistoryLoader, CollectionMintTransfersLoader, CollectionMintsLoader,
    CollectionMintsOwnerLoader, CollectionSupplyLoader, CollectionTotalMintsLoader, CreatorsLoader,
    DropLoader, DropMintHistoryLoader, HoldersLoader, MetadataJsonAttributesLoader,
    MetadataJsonLoader, MintCreatorsLoader, MinterMintHistoryLoader, ProjectCollectionLoader,
    ProjectCollectionsLoader, ProjectDropsLoader, QueuedMintsLoader, SwitchCollectionHistoryLoader,
    UpdateMintHistoryLoader,
};
use db::Connection;
use hub_core::{
    anyhow::{Error, Result},
    assets::AssetProxy,
    clap,
    consumer::RecvError,
    credits::CreditsClient,
    prelude::*,
    producer::Producer,
    tokio,
    uuid::Uuid,
};
use metrics::Metrics;
use mutations::Mutation;
use poem::{async_trait, FromRequest, Request, RequestBody};
use queries::Query;
use redis::Client as Redis;

#[allow(clippy::pedantic)]
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/nfts.proto.rs"));
    include!(concat!(env!("OUT_DIR"), "/treasury.proto.rs"));
    include!(concat!(env!("OUT_DIR"), "/solana_nfts.proto.rs"));
    include!(concat!(env!("OUT_DIR"), "/polygon_nfts.proto.rs"));
}

use proto::NftEvents;

impl hub_core::producer::Message for proto::NftEvents {
    type Key = proto::NftEventKey;
}

#[derive(Debug, Clone)]
pub enum Services {
    Treasury(proto::TreasuryEventKey, proto::TreasuryEvents),
    Solana(proto::SolanaNftEventKey, proto::SolanaNftEvents),
    Polygon(proto::PolygonNftEventKey, proto::PolygonNftEvents),
}

impl hub_core::consumer::MessageGroup for Services {
    const REQUESTED_TOPICS: &'static [&'static str] =
        &["hub-treasuries", "hub-nfts-solana", "hub-nfts-polygon"];

    fn from_message<M: hub_core::consumer::Message>(msg: &M) -> Result<Self, RecvError> {
        let topic = msg.topic();
        let key = msg.key().ok_or(RecvError::MissingKey)?;
        let val = msg.payload().ok_or(RecvError::MissingPayload)?;
        info!(topic, ?key, ?val);

        match topic {
            "hub-treasuries" => {
                let key = proto::TreasuryEventKey::decode(key)?;
                let val = proto::TreasuryEvents::decode(val)?;

                Ok(Services::Treasury(key, val))
            },
            "hub-nfts-solana" => {
                let key = proto::SolanaNftEventKey::decode(key)?;
                let val = proto::SolanaNftEvents::decode(val)?;

                Ok(Services::Solana(key, val))
            },
            "hub-nfts-polygon" => {
                let key = proto::PolygonNftEventKey::decode(key)?;
                let val = proto::PolygonNftEvents::decode(val)?;

                Ok(Services::Polygon(key, val))
            },
            t => Err(RecvError::BadTopic(t.into())),
        }
    }
}

pub type AppSchema = Schema<Query, Mutation, EmptySubscription>;

#[derive(Debug, clap::Args)]
#[command(version, author, about)]
pub struct Args {
    #[arg(short, long, env, default_value_t = 3004)]
    pub port: u16,

    #[command(flatten)]
    pub db: db::DbArgs,

    #[command(flatten)]
    pub hub_uploads: hub_uploads::HubUploadArgs,

    #[arg(long, env)]
    pub redis_url: String,

    #[command(subcommand)]
    pub command: Option<Subcommand>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    RetryJobs,
}

#[derive(Debug, Clone, Copy)]
pub struct UserID(Option<Uuid>);

impl TryFrom<&str> for UserID {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        let id = Uuid::from_str(value)?;

        Ok(Self(Some(id)))
    }
}

#[async_trait]
impl<'a> FromRequest<'a> for UserID {
    async fn from_request(req: &'a Request, _body: &mut RequestBody) -> poem::Result<Self> {
        let id = req
            .headers()
            .get("X-USER-ID")
            .and_then(|value| value.to_str().ok())
            .map_or(Ok(Self(None)), Self::try_from)?;

        Ok(id)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OrganizationId(Option<Uuid>);

impl TryFrom<&str> for OrganizationId {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        let id = Uuid::from_str(value)?;

        Ok(Self(Some(id)))
    }
}

#[async_trait]
impl<'a> FromRequest<'a> for OrganizationId {
    async fn from_request(req: &'a Request, _body: &mut RequestBody) -> poem::Result<Self> {
        let id = req
            .headers()
            .get("X-ORGANIZATION-ID")
            .and_then(|value| value.to_str().ok())
            .map_or(Ok(Self(None)), Self::try_from)?;

        Ok(id)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Balance(Option<u64>);

impl TryFrom<&str> for Balance {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        let balance = value.parse()?;

        Ok(Self(Some(balance)))
    }
}

#[async_trait]
impl<'a> FromRequest<'a> for Balance {
    async fn from_request(req: &'a Request, _body: &mut RequestBody) -> poem::Result<Self> {
        let id = req
            .headers()
            .get("X-CREDIT-BALANCE")
            .and_then(|value| value.to_str().ok())
            .map_or(Ok(Self(None)), Self::try_from)?;

        Ok(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::EnumIter, strum::AsRefStr)]
pub enum Actions {
    CreateDrop,
    MintEdition,
    RetryMint,
    RetryDrop,
    TransferAsset,
    Mint,
    MintCompressed,
    CreateCollection,
    RetryCollection,
    UpdateMint,
}

impl From<Actions> for hub_core::credits::Action {
    fn from(value: Actions) -> Self {
        match value {
            Actions::CreateDrop => hub_core::credits::Action::CreateDrop,
            Actions::MintEdition => hub_core::credits::Action::MintEdition,
            Actions::RetryMint => hub_core::credits::Action::RetryMint,
            Actions::RetryDrop => hub_core::credits::Action::RetryDrop,
            Actions::TransferAsset => hub_core::credits::Action::TransferAsset,
            Actions::Mint => hub_core::credits::Action::Mint,
            Actions::MintCompressed => hub_core::credits::Action::MintCompressed,
            Actions::CreateCollection => hub_core::credits::Action::CreateCollection,
            Actions::RetryCollection => hub_core::credits::Action::RetryCollection,
            Actions::UpdateMint => hub_core::credits::Action::UpdateMint,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub schema: AppSchema,
    pub connection: Connection,
    pub producer: Producer<NftEvents>,
    pub credits: CreditsClient<Actions>,
    pub solana: Solana,
    pub polygon: Polygon,
    pub asset_proxy: AssetProxy,
    pub metadata_json_upload_job_queue: JobQueue,
    pub redis: Redis,
}

impl AppState {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema: AppSchema,
        connection: Connection,
        producer: Producer<NftEvents>,
        credits: CreditsClient<Actions>,
        solana: Solana,
        polygon: Polygon,
        asset_proxy: AssetProxy,
        metadata_json_upload_job_queue: JobQueue,
        redis: Redis,
    ) -> Self {
        Self {
            schema,
            connection,
            producer,
            credits,
            solana,
            polygon,
            asset_proxy,
            metadata_json_upload_job_queue,
            redis,
        }
    }
}

pub struct AppContext {
    pub db: Connection,
    user_id: UserID,
    organization_id: OrganizationId,
    balance: Balance,
    redis: Redis,
    project_drops_loader: DataLoader<ProjectDropsLoader>,
    project_collections_loader: DataLoader<ProjectCollectionsLoader>,
    project_collection_loader: DataLoader<ProjectCollectionLoader>,
    collection_loader: DataLoader<CollectionLoader>,
    metadata_json_loader: DataLoader<MetadataJsonLoader>,
    metadata_json_attributes_loader: DataLoader<MetadataJsonAttributesLoader>,
    collection_mints_loader: DataLoader<CollectionMintsLoader>,
    single_collection_mint_loader: DataLoader<CollectionMintLoader>,
    collection_mints_owner_loader: DataLoader<CollectionMintsOwnerLoader>,
    collection_drop_loader: DataLoader<CollectionDropLoader>,
    drop_loader: DataLoader<DropLoader>,
    creators_loader: DataLoader<CreatorsLoader>,
    holders_loader: DataLoader<HoldersLoader>,
    collection_mint_history_loader: DataLoader<CollectionMintHistoriesLoader>,
    drop_mint_history_loader: DataLoader<DropMintHistoryLoader>,
    minter_mint_history_loader: DataLoader<MinterMintHistoryLoader>,
    update_mint_history_loader: DataLoader<UpdateMintHistoryLoader>,
    mint_creators_loader: DataLoader<MintCreatorsLoader>,
    collection_mint_mint_history_loader: DataLoader<CollectionMintMintHistoryLoader>,
    collection_mint_transfers_loader: DataLoader<CollectionMintTransfersLoader>,
    switch_collection_history_loader: DataLoader<SwitchCollectionHistoryLoader>,
    queued_mints_loader: DataLoader<QueuedMintsLoader>,
    collection_total_mints_loader: DataLoader<CollectionTotalMintsLoader>,
    collection_supply_loader: DataLoader<CollectionSupplyLoader>,
}

impl AppContext {
    #[must_use]
    #[allow(clippy::similar_names)]
    pub fn new(
        db: Connection,
        redis: Redis,
        user_id: UserID,
        organization_id: OrganizationId,
        balance: Balance,
    ) -> Self {
        let project_drops_loader =
            DataLoader::new(ProjectDropsLoader::new(db.clone()), tokio::spawn);
        let collection_loader = DataLoader::new(CollectionLoader::new(db.clone()), tokio::spawn);
        let project_collections_loader =
            DataLoader::new(ProjectCollectionsLoader::new(db.clone()), tokio::spawn);
        let project_collection_loader =
            DataLoader::new(ProjectCollectionLoader::new(db.clone()), tokio::spawn);
        let metadata_json_loader =
            DataLoader::new(MetadataJsonLoader::new(db.clone()), tokio::spawn);
        let metadata_json_attributes_loader =
            DataLoader::new(MetadataJsonAttributesLoader::new(db.clone()), tokio::spawn);
        let collection_mints_loader =
            DataLoader::new(CollectionMintsLoader::new(db.clone()), tokio::spawn);
        let collection_mints_owner_loader =
            DataLoader::new(CollectionMintsOwnerLoader::new(db.clone()), tokio::spawn);
        let collection_drop_loader: DataLoader<_> =
            DataLoader::new(CollectionDropLoader::new(db.clone()), tokio::spawn);
        let drop_loader = DataLoader::new(DropLoader::new(db.clone()), tokio::spawn);
        let creators_loader = DataLoader::new(CreatorsLoader::new(db.clone()), tokio::spawn);
        let holders_loader = DataLoader::new(HoldersLoader::new(db.clone()), tokio::spawn);
        let collection_mint_history_loader: DataLoader<CollectionMintHistoriesLoader> =
            DataLoader::new(CollectionMintHistoriesLoader::new(db.clone()), tokio::spawn);
        let drop_mint_history_loader =
            DataLoader::new(DropMintHistoryLoader::new(db.clone()), tokio::spawn);
        let single_collection_mint_loader =
            DataLoader::new(CollectionMintLoader::new(db.clone()), tokio::spawn);
        let minter_mint_history_loader =
            DataLoader::new(MinterMintHistoryLoader::new(db.clone()), tokio::spawn);
        let update_mint_history_loader =
            DataLoader::new(UpdateMintHistoryLoader::new(db.clone()), tokio::spawn);
        let mint_creators_loader =
            DataLoader::new(MintCreatorsLoader::new(db.clone()), tokio::spawn);
        let collection_mint_mint_history_loader = DataLoader::new(
            CollectionMintMintHistoryLoader::new(db.clone()),
            tokio::spawn,
        );
        let collection_mint_transfers_loader =
            DataLoader::new(CollectionMintTransfersLoader::new(db.clone()), tokio::spawn);
        let switch_collection_history_loader =
            DataLoader::new(SwitchCollectionHistoryLoader::new(db.clone()), tokio::spawn);
        let queued_mints_loader = DataLoader::new(QueuedMintsLoader::new(db.clone()), tokio::spawn);
        let collection_total_mints_loader = DataLoader::new(
            CollectionTotalMintsLoader::new(db.clone(), redis.clone()),
            tokio::spawn,
        );
        let collection_supply_loader = DataLoader::new(
            CollectionSupplyLoader::new(db.clone(), redis.clone()),
            tokio::spawn,
        );

        Self {
            db,
            user_id,
            organization_id,
            balance,
            redis,
            project_drops_loader,
            project_collections_loader,
            project_collection_loader,
            collection_loader,
            metadata_json_loader,
            metadata_json_attributes_loader,
            collection_mints_loader,
            single_collection_mint_loader,
            collection_mints_owner_loader,
            collection_drop_loader,
            drop_loader,
            creators_loader,
            holders_loader,
            collection_mint_history_loader,
            drop_mint_history_loader,
            minter_mint_history_loader,
            update_mint_history_loader,
            mint_creators_loader,
            collection_mint_mint_history_loader,
            collection_mint_transfers_loader,
            switch_collection_history_loader,
            queued_mints_loader,
            collection_total_mints_loader,
            collection_supply_loader,
        }
    }
}

/// Builds the GraphQL Schema, attaching the Database to the context
#[must_use]
pub fn build_schema() -> AppSchema {
    Schema::build(Query::default(), Mutation::default(), EmptySubscription)
        .extension(ApolloTracing)
        .extension(Logger)
        .enable_federation()
        .finish()
}
