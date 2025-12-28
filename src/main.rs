use std::sync::{Arc, Mutex};
use tracing_subscriber::{self, layer::SubscriberExt, util::SubscriberInitExt};

use notes::{create_app, AppState, Note};

const APP_NAME: &str = "notes";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from(format!(
            "RUST_LOG={},{}=debug,tower_http=debug,axum::rejection=trace",
            std::env::var("RUST_LOG").unwrap_or("info".to_string()),
            env!("CARGO_CRATE_NAME")
        )))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let notes = Arc::new(Mutex::new(Vec::<Note>::new()));

    let host_port = "0.0.0.0:3000";
    let api_version = "v1";
    let notes_path = format!("{}/{}/notes", host_port, api_version);

    let app = create_app(Arc::new(AppState { notes, notes_path }), api_version);

    let span = tracing::info_span!(
        "Start app",
        app = APP_NAME,
        api_version = api_version
    );
    let _enter = span.enter();
    tracing::debug!("Setup listener on {}", host_port);
    let listener = match tokio::net::TcpListener::bind(host_port).await {
        Ok(listener) => listener,
        Err(err) => {
            tracing::error!("unable to setup lister {}", host_port);
            return Err(err.into());
        }
    };
    tracing::info!("Serve on {}", host_port);
    if let Err(err) = axum::serve(listener, app).await {
        tracing::error!("unable to serve app for listener at {}", host_port);
        return Err(err.into());
    }
    Ok(())
}
