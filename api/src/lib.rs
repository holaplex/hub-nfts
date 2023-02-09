#![deny(clippy::disallowed_methods, clippy::suspicious, clippy::style)]
#![warn(clippy::pedantic, clippy::cargo)]
#![allow(clippy::module_name_repetitions)]

pub mod api;
pub mod db;
pub mod entities;
pub mod handlers;
use db::Connection;
use hub_core::{clap, prelude::*, producer::Producer};
use solana_client::rpc_client::RpcClient;

#[derive(Debug, clap::Args)]
#[command(version, author, about)]
pub struct Args {
    #[arg(short, long, env, default_value_t = 3002)]
    pub port: u16,

    #[arg(short, long, env)]
    pub solana_endpoint: String,

    #[command(flatten)]
    pub db: db::DbArgs,
}

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/drops.proto.rs"));
}

use proto::DropEvents;

impl hub_core::producer::Message for proto::DropEvents {
    type Key = proto::DropEventKey;
}

#[derive(Clone)]
pub struct AppState {
    pub connection: Connection,
    pub rpc: Arc<RpcClient>,
    pub producer: Producer<DropEvents>,
}

impl AppState {
    #[must_use]
    pub fn new(
        connection: Connection,
        rpc: Arc<RpcClient>,
        producer: Producer<DropEvents>,
    ) -> Self {
        Self {
            connection,
            rpc,
            producer,
        }
    }
}
