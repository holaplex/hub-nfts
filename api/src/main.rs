//!

use async_graphql::futures_util::StreamExt;
use holaplex_hub_nfts::{
    blockchains::{polygon::Polygon, solana::Solana},
    build_schema,
    db::Connection,
    events,
    handlers::{graphql_handler, health, playground},
    nft_storage::NftStorageClient,
    proto, Actions, AppState, Args, Services,
};
use hub_core::{
    anyhow::Context as AnyhowContext,
    tokio::{self, task},
    tracing::{info, warn},
};
use poem::{get, listener::TcpListener, middleware::AddData, post, EndpointExt, Route, Server};

pub fn main() {
    let opts = hub_core::StartConfig {
        service_name: "hub-nfts",
    };

    hub_core::run(opts, |common, args| {
        let Args {
            port,
            db,
            nft_storage,
        } = args;

        common.rt.block_on(async move {
            let connection = Connection::new(db)
                .await
                .context("failed to get database connection")?;

            let schema = build_schema();

            let producer = common
                .producer_cfg
                .clone()
                .build::<proto::NftEvents>()
                .await?;
            let solana_producer = common
                .producer_cfg
                .clone()
                .build::<proto::SolanaEvents>()
                .await?;
            let polygon_producer = common.producer_cfg.build::<proto::PolygonEvents>().await?;
            let credits = common.credits_cfg.build::<Actions>().await?;
            let nft_storage = NftStorageClient::new(nft_storage)?;
            let event_processor =
                events::Processor::new(connection.clone(), credits.clone(), producer.clone());

            let solana = Solana::new(solana_producer.clone());
            let polygon = Polygon::new(polygon_producer.clone());

            let state = AppState::new(
                schema,
                connection.clone(),
                producer.clone(),
                credits.clone(),
                solana,
                polygon,
                nft_storage,
                common.asset_proxy,
            );

            let cons = common.consumer_cfg.build::<Services>().await?;

            tokio::spawn(async move {
                {
                    let mut stream = cons.stream();
                    loop {
                        let event_processor = event_processor.clone();

                        match stream.next().await {
                            Some(Ok(msg)) => {
                                info!(?msg, "message received");

                                tokio::spawn(async move { event_processor.process(msg).await });
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
