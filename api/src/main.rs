//!

use holaplex_hub_nfts::{
    background_worker::{
        job_queue::JobQueue,
        tasks::{MetadataJsonUploadContext, MetadataJsonUploadTask},
        worker::Worker,
    },
    blockchains::{polygon::Polygon, solana::Solana},
    build_schema,
    db::Connection,
    events,
    handlers::{graphql_handler, health, metrics_handler, playground},
    hub_uploads::HubUploadClient,
    metrics::Metrics,
    proto, Actions, AppState, Args, Services,
};
use hub_core::{prelude::*, tokio};
use poem::{get, listener::TcpListener, middleware::AddData, post, EndpointExt, Route, Server};
use redis::Client as RedisClient;

pub fn main() {
    let opts = hub_core::StartConfig {
        service_name: "hub-nfts",
    };

    hub_core::run(opts, |common, args| {
        let Args {
            port,
            db,
            hub_uploads,
            redis_url,
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
            let hub_uploads = HubUploadClient::new(hub_uploads)?;

            let metrics = Metrics::new()?;

            let event_processor = events::Processor::new(
                connection.clone(),
                credits.clone(),
                producer.clone(),
                metrics.clone(),
            );

            let solana = Solana::new(producer.clone());
            let polygon = Polygon::new(producer.clone());

            let redis_client = RedisClient::open(redis_url)?;

            let metadata_json_upload_task_context =
                MetadataJsonUploadContext::new(hub_uploads, solana.clone(), polygon.clone());

            let job_queue = JobQueue::new(redis_client.clone(), connection.clone());
            let worker = Worker::<MetadataJsonUploadContext, MetadataJsonUploadTask>::new(
                job_queue.clone(),
                connection.clone(),
                metadata_json_upload_task_context,
            );

            let state = AppState::new(
                schema,
                connection.clone(),
                producer.clone(),
                credits.clone(),
                solana.clone(),
                polygon.clone(),
                common.asset_proxy,
                job_queue.clone(),
                redis_client,
            );

            let cons = common.consumer_cfg.build::<Services>().await?;

            tokio::spawn(async move { worker.start().await });

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
                        .at(
                            "/graphql",
                            post(graphql_handler).with(AddData::new(state.clone())),
                        )
                        .at("/playground", get(playground))
                        .at("/health", get(health))
                        .at("/metrics", get(metrics_handler).with(AddData::new(metrics))),
                )
                .await
                .context("failed to build graphql server")
        })
    });
}
