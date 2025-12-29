use notes::{create_app, AppConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::var("NOTES_HOST").unwrap_or("0.0.0.0".to_string());
    let port = std::env::var("NOTES_PORT").unwrap_or("3000".to_string());
    let db_uri = std::env::var("NOTES_DB_ADDRESS").unwrap_or("uri".to_string());
    create_app(AppConfig {
        host_port: format!("{}:{}", host, port).to_string(),
        api_version: "v1".to_string(),
        db_uri,
    })
    .await?;
    Ok(())
}
