use hub_core::{
    anyhow::{anyhow, Result},
    metrics::*,
};
use once_cell::sync::Lazy;

pub static HANDLER_ALL: Lazy<[KeyValue; 1]> = Lazy::new(|| [KeyValue::new("handler", "all")]);

#[derive(Clone)]
pub struct Metrics {
    pub registry: Registry,
    pub provider: MeterProvider,
    pub mint_duration_ms_bucket: Histogram<i64>,
}

impl Metrics {
    pub fn new() -> Result<Self> {
        let registry = Registry::new();
        let exporter = hub_core::metrics::exporter()
            .with_registry(registry.clone())
            .with_namespace("hub_nfts")
            .build()
            .map_err(|e| anyhow!("Failed to build exporter: {}", e))?;

        let provider = MeterProvider::builder()
            .with_reader(exporter)
            .with_resource(Resource::new(vec![KeyValue::new(
                "service.name",
                "hub-nfts",
            )]))
            .build();

        let meter = provider.meter("hub-nfts");

        let mint_duration_ms_bucket = meter
            .i64_histogram("mint.time")
            .with_unit(Unit::new("ms"))
            .with_description("Mint duration time in milliseconds.")
            .init();

        Ok(Self {
            registry,
            provider,
            mint_duration_ms_bucket,
        })
    }
}
