use std::{env, sync::Once};
use tracing::Level;
use tracing_forest::ForestLayer;
use tracing_subscriber::{
    EnvFilter, Layer, Registry, filter::filter_fn, fmt::format::FmtSpan, layer::SubscriberExt,
    util::SubscriberInitExt,
};

static INIT: Once = Once::new();

pub fn setup_logger() {
    INIT.call_once(|| {
        let default_filter = "off";
        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

        let logger_type = env::var("RUST_LOGGER").unwrap_or_else(|_| "flat".to_string());
        match logger_type.as_str() {
            "forest" => {
                Registry::default()
                    .with(env_filter)
                    .with(ForestLayer::default().with_filter(filter_fn(|metadata| {
                        metadata.is_span() || metadata.level() == &Level::INFO
                    })))
                    .init();
            }
            "forest-all" => {
                Registry::default()
                    .with(env_filter)
                    .with(ForestLayer::default())
                    .init();
            }
            "flat" => {
                tracing_subscriber::fmt::Subscriber::builder()
                    .compact()
                    .with_ansi(false)
                    .with_file(false)
                    .with_target(false)
                    .with_thread_names(false)
                    .with_env_filter(env_filter)
                    .with_span_events(FmtSpan::CLOSE)
                    .finish()
                    .init();
            }
            _ => {
                panic!("Invalid logger type: {}", logger_type);
            }
        }
    });
}
