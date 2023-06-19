#![deny(clippy::disallowed_methods, clippy::suspicious, clippy::style)]
#![warn(clippy::pedantic, clippy::cargo)]
#![allow(clippy::module_name_repetitions)]

pub mod blockchains;
pub mod collection;
pub mod dataloaders;
pub mod db;
pub mod entities;
pub mod events;
pub mod handlers;
pub mod metadata_json;
pub mod mutations;
pub mod nft_storage;
pub mod objects;
pub mod queries;

use async_graphql::{
    dataloader::DataLoader,
    extensions::{ApolloTracing, Logger},
    EmptySubscription, Schema,
};
use blockchains::{polygon::Polygon, solana::Solana};
use dataloaders::{
    CollectionLoader, CollectionMintsLoader, CollectionMintsOwnerLoader, CollectionPurchasesLoader,
    CreatorsLoader, DropLoader, DropPurchasesLoader, HoldersLoader, MetadataJsonAttributesLoader,
    MetadataJsonLoader, ProjectDropsLoader,
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
use mutations::Mutation;
use nft_storage::NftStorageClient;
use poem::{async_trait, FromRequest, Request, RequestBody};
use queries::Query;

#[allow(clippy::pedantic)]
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/nfts.proto.rs"));
    include!(concat!(env!("OUT_DIR"), "/treasury.proto.rs"));
    include!(concat!(env!("OUT_DIR"), "/solana_nfts.proto.rs"));
}

use proto::NftEvents;

impl hub_core::producer::Message for proto::NftEvents {
    type Key = proto::NftEventKey;
}

impl hub_core::producer::Message for proto::SolanaEvents {
    type Key = proto::NftEventKey;
}

impl hub_core::producer::Message for proto::PolygonEvents {
    type Key = proto::NftEventKey;
}

#[derive(Debug)]
pub enum Services {
    Treasury(proto::TreasuryEventKey, proto::TreasuryEvents),
    Solana(proto::SolanaNftEventKey, proto::SolanaNftEvents),
}

impl hub_core::consumer::MessageGroup for Services {
    const REQUESTED_TOPICS: &'static [&'static str] = &["hub-treasuries", "hub-nfts-solana"];

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
    pub nft_storage: nft_storage::NftStorageArgs,
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
}

impl From<Actions> for hub_core::credits::Action {
    fn from(value: Actions) -> Self {
        match value {
            Actions::CreateDrop => hub_core::credits::Action::CreateDrop,
            Actions::MintEdition => hub_core::credits::Action::MintEdition,
            Actions::RetryMint => hub_core::credits::Action::RetryMint,
            Actions::RetryDrop => hub_core::credits::Action::RetryDrop,
            Actions::TransferAsset => hub_core::credits::Action::TransferAsset,
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
    pub nft_storage: NftStorageClient,
    pub asset_proxy: AssetProxy,
}

impl AppState {
    #[must_use]
    pub fn new(
        schema: AppSchema,
        connection: Connection,
        producer: Producer<NftEvents>,
        credits: CreditsClient<Actions>,
        solana: Solana,
        polygon: Polygon,
        nft_storage: NftStorageClient,
        asset_proxy: AssetProxy,
    ) -> Self {
        Self {
            schema,
            connection,
            producer,
            credits,
            solana,
            polygon,
            nft_storage,
            asset_proxy,
        }
    }
}

pub struct AppContext {
    pub db: Connection,
    user_id: UserID,
    organization_id: OrganizationId,
    balance: Balance,
    project_drops_loader: DataLoader<ProjectDropsLoader>,
    collection_loader: DataLoader<CollectionLoader>,
    metadata_json_loader: DataLoader<MetadataJsonLoader>,
    metadata_json_attributes_loader: DataLoader<MetadataJsonAttributesLoader>,
    collection_mints_loader: DataLoader<CollectionMintsLoader>,
    collection_mints_owner_loader: DataLoader<CollectionMintsOwnerLoader>,
    drop_loader: DataLoader<DropLoader>,
    creators_loader: DataLoader<CreatorsLoader>,
    holders_loader: DataLoader<HoldersLoader>,
    collection_purchases_loader: DataLoader<CollectionPurchasesLoader>,
    drop_purchases_loader: DataLoader<DropPurchasesLoader>,
}

impl AppContext {
    #[must_use]
    pub fn new(
        db: Connection,
        user_id: UserID,
        organization_id: OrganizationId,
        balance: Balance,
    ) -> Self {
        let project_drops_loader =
            DataLoader::new(ProjectDropsLoader::new(db.clone()), tokio::spawn);
        let collection_loader = DataLoader::new(CollectionLoader::new(db.clone()), tokio::spawn);
        let metadata_json_loader =
            DataLoader::new(MetadataJsonLoader::new(db.clone()), tokio::spawn);
        let metadata_json_attributes_loader =
            DataLoader::new(MetadataJsonAttributesLoader::new(db.clone()), tokio::spawn);
        let collection_mints_loader =
            DataLoader::new(CollectionMintsLoader::new(db.clone()), tokio::spawn);
        let collection_mints_owner_loader =
            DataLoader::new(CollectionMintsOwnerLoader::new(db.clone()), tokio::spawn);
        let drop_loader = DataLoader::new(DropLoader::new(db.clone()), tokio::spawn);
        let creators_loader = DataLoader::new(CreatorsLoader::new(db.clone()), tokio::spawn);
        let holders_loader = DataLoader::new(HoldersLoader::new(db.clone()), tokio::spawn);
        let collection_purchases_loader =
            DataLoader::new(CollectionPurchasesLoader::new(db.clone()), tokio::spawn);
        let drop_purchases_loader =
            DataLoader::new(DropPurchasesLoader::new(db.clone()), tokio::spawn);

        Self {
            db,
            user_id,
            organization_id,
            balance,
            project_drops_loader,
            collection_loader,
            metadata_json_loader,
            metadata_json_attributes_loader,
            collection_mints_loader,
            collection_mints_owner_loader,
            drop_loader,
            creators_loader,
            holders_loader,
            collection_purchases_loader,
            drop_purchases_loader,
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
