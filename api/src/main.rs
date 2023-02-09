//!

use std::sync::Arc;

use holaplex_hub_drops::{api::NftApi, db::Connection, handlers::health, proto, AppState, Args};
use hub_core::anyhow::Context as AnyhowContext;
use poem::{get, listener::TcpListener, middleware::AddData, EndpointExt, Route, Server};
use poem_openapi::OpenApiService;
use solana_client::rpc_client::RpcClient;

pub fn main() {
    let opts = hub_core::StartConfig {
        service_name: "hub-drops",
    };

    hub_core::run(opts, |common, args| {
        let Args {
            port,
            solana_endpoint,
            db,
        } = args;

        common.rt.block_on(async move {
            let connection = Connection::new(db)
                .await
                .context("failed to get database connection")?;
            let rpc = RpcClient::new(solana_endpoint);

            let api_service = OpenApiService::new(NftApi, "HubDrops", "0.1.0")
                .server(format!("http://localhost:{port}/v1"));
            let ui = api_service.swagger_ui();
            let spec = api_service.spec_endpoint();

            let producer = common.producer_cfg.build::<proto::DropEvents>().await?;

            let state = AppState::new(connection, Arc::new(rpc), producer);

            Server::new(TcpListener::bind(format!("0.0.0.0:{port}")))
                .run(
                    Route::new()
                        .nest("/v1", api_service.with(AddData::new(state)))
                        .nest("/", ui)
                        .at("/spec", spec)
                        .at("/health", get(health)),
                )
                .await
                .context("failed to build graphql server")
        })
    });
}
