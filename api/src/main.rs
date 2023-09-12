//!

use holaplex_hub_nfts::{
    blockchains::{polygon::Polygon, solana::Solana},
    build_schema,
    db::Connection,
    events,
    handlers::{graphql_handler, health, playground},
    metadata_json,
    nft_storage::NftStorageClient,
    proto, Actions, AppState, Args, Services,
};
use hub_core::{prelude::*, tokio};
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
            let credits = common.credits_cfg.build::<Actions>().await?;
            let nft_storage = NftStorageClient::new(nft_storage)?;
            let event_processor =
                events::Processor::new(connection.clone(), credits.clone(), producer.clone());

            let solana = Solana::new(producer.clone());
            let polygon = Polygon::new(producer.clone());

            let (mj_job_runner, _mj_job_task) = metadata_json::JobRunner::new(
                metadata_json::JobRunnerContext {
                    db: connection.clone(),
                    solana: solana.clone(),
                    polygon: polygon.clone(),
                    nfts_producer: producer.clone(),
                },
                nft_storage,
            );

            let state = AppState::new(
                schema,
                connection.clone(),
                producer.clone(),
                credits.clone(),
                solana,
                polygon,
                mj_job_runner,
                common.asset_proxy,
            );

            let cons = common.consumer_cfg.build::<Services>().await?;

            tokio::spawn(async move {
                cons.consume(
                    |b| {
                        b.with_jitter()
                            .with_min_delay(Duration::from_millis(500))
                            .with_max_delay(Duration::from_secs(90))
                    },
                    |e| async move { event_processor.process(e).await },
                )
                .await
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
