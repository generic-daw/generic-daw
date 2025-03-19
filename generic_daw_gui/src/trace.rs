use tracing_subscriber::EnvFilter;

const DEFAULT_LOG_FILTER: &str = "none,clap_host=trace";

pub fn setup() {
    let directives = std::env::var("RUST_LOG").unwrap_or_else(|_| DEFAULT_LOG_FILTER.to_owned());

    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(EnvFilter::builder().parse_lossy(directives))
        .init();
}
