//!

use std::sync::Arc;

use async_graphql::futures_util::StreamExt;
use holaplex_hub_drops::{
    build_schema,
    db::Connection,
    events,
    handlers::{graphql_handler, health, playground},
    proto, AppState, Args, Services,
};
use hub_core::{
    anyhow::Context as AnyhowContext,
    tokio::{self, task},
    tracing::{info, warn},
};
use poem::{get, listener::TcpListener, middleware::AddData, post, EndpointExt, Route, Server};
use solana_client::rpc_client::RpcClient;

pub fn main() {
    let opts = hub_core::StartConfig {
        service_name: "hub-drops",
    };

    hub_core::run(opts, |common, args| {
        let Args {
            port,
            solana_endpoint,
            keypair_path,
            db,
        } = args;

        common.rt.block_on(async move {
            let connection = Connection::new(db)
                .await
                .context("failed to get database connection")?;
            let rpc = RpcClient::new(solana_endpoint);
            let schema = build_schema();
            let producer = common.producer_cfg.build::<proto::DropEvents>().await?;
            let state = AppState::new(
                schema,
                connection.clone(),
                Arc::new(rpc),
                producer.clone(),
                keypair_path,
            );

            let cons = common.consumer_cfg.build::<Services>().await?;

            tokio::spawn(async move {
                {
                    let mut stream = cons.stream();
                    loop {
                        let connection = connection.clone();

                        match stream.next().await {
                            Some(Ok(msg)) => {
                                info!(?msg, "message received");

                                tokio::spawn(async move {
                                    events::process(msg, connection.clone()).await
                                });
                                task::yield_now().await;
                            },
                            None => (),
                            Some(Err(e)) => {
                                warn!("failed to get message {:?}", e);
                            },
                        }
                    }
                }
            });

            Server::new(TcpListener::bind(format!("0.0.0.0:{port}")))
                .run(
                    Route::new()
                        .at("/graphql", post(graphql_handler).with(AddData::new(state)))
                        .at("/playground", get(playground))
                        .at("/health", get(health)),
                )
                .await
                .context("failed to build graphql server")
        })
    });
}
