#![deny(clippy::disallowed_methods, clippy::suspicious, clippy::style)]
#![warn(clippy::pedantic, clippy::cargo)]
#![allow(clippy::module_name_repetitions)]

pub mod dataloaders;
pub mod db;
pub mod entities;
pub mod events;
pub mod handlers;
pub mod objects;
pub mod nft_storage;

use std::fs::File;

use db::Connection;
pub mod mutations;
pub mod queries;
use async_graphql::{
    dataloader::DataLoader,
    extensions::{ApolloTracing, Logger},
    EmptySubscription, Schema,
};
use dataloaders::{CollectionLoader, ProjectDropsLoader};
use hub_core::{
    anyhow::{Error, Result},
    clap,
    consumer::RecvError,
    prelude::*,
    producer::Producer,
    serde_json, tokio,
    uuid::Uuid,
};
use mutations::Mutation;
use nft_storage::NftStorageClient;
use poem::{async_trait, FromRequest, Request, RequestBody};
use queries::Query;
use solana_client::rpc_client::RpcClient;

#[allow(clippy::pedantic)]
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/nfts.proto.rs"));
    include!(concat!(env!("OUT_DIR"), "/treasury.proto.rs"));
}

use proto::NftEvents;

impl hub_core::producer::Message for proto::NftEvents {
    type Key = proto::NftEventKey;
}

#[derive(Debug)]
pub enum Services {
    Treasuries(proto::TreasuryEventKey, proto::TreasuryEvents),
}

impl hub_core::consumer::MessageGroup for Services {
    const REQUESTED_TOPICS: &'static [&'static str] = &["hub-treasuries"];

    fn from_message<M: hub_core::consumer::Message>(msg: &M) -> Result<Self, RecvError> {
        let topic = msg.topic();
        let key = msg.key().ok_or(RecvError::MissingKey)?;
        let val = msg.payload().ok_or(RecvError::MissingPayload)?;
        info!(topic, ?key, ?val);

        match topic {
            "hub-treasuries" => {
                let key = proto::TreasuryEventKey::decode(key)?;
                let val = proto::TreasuryEvents::decode(val)?;

                Ok(Services::Treasuries(key, val))
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

    #[arg(short, long, env)]
    pub solana_endpoint: String,

    #[arg(short, long, env)]
    pub keypair_path: String,

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

#[derive(Clone)]
pub struct AppState {
    pub schema: AppSchema,
    pub connection: Connection,
    pub rpc: Arc<RpcClient>,
    pub producer: Producer<NftEvents>,
    pub keypair: Vec<u8>,
    pub nft_storage: NftStorageClient,
}

impl AppState {
    #[must_use]
    pub fn new(
        schema: AppSchema,
        connection: Connection,
        rpc: Arc<RpcClient>,
        producer: Producer<NftEvents>,
        path: String,
        nft_storage: NftStorageClient,
    ) -> Self {
        let f = File::open(path).expect("unable to locate keypair file");
        let keypair: Vec<u8> =
            serde_json::from_reader(f).expect("unable to read keypair bytes from the file");

        Self {
            schema,
            connection,
            rpc,
            producer,
            keypair,
            nft_storage,
        }
    }
}

pub struct AppContext {
    pub db: Connection,
    user_id: UserID,
    project_drops_loader: DataLoader<ProjectDropsLoader>,
    collection_loader: DataLoader<CollectionLoader>,
}

impl AppContext {
    #[must_use]
    pub fn new(db: Connection, user_id: UserID) -> Self {
        let project_drops_loader =
            DataLoader::new(ProjectDropsLoader::new(db.clone()), tokio::spawn);

        let collection_loader = DataLoader::new(CollectionLoader::new(db.clone()), tokio::spawn);

        Self {
            db,
            user_id,
            project_drops_loader,
            collection_loader,
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
