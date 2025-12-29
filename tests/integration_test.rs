use testcontainers::{clients, GenericImage, RunnableImage};

#[tokio::test]
async fn test_integration() {
    // Start Docker client
    let docker = clients::Cli::default();

    // Start MongoDB container
    let mongo_port = 27017;
    let mongo_image = RunnableImage::from(
        GenericImage::new("mongo", "7.0.5").with_exposed_port(mongo_port),
    );
    let _ = docker.run(mongo_image);

    // Build the MongoDB connection string
    let uri = format!("mongodb://mongo:{}", mongo_port);

    // Start your app container (assuming your image is built and named "rust-notes")
    let app_image = RunnableImage::from(
        GenericImage::new("devchris123/rust-notes", "latest")
            .with_env_var("NOTES_HOST".to_string(), "0.0.0.0".to_string())
            .with_env_var("NOTES_PORT".to_string(), "3000".to_string())
            .with_env_var("NOTES_DB_ADDRESS".to_string(), uri.clone())
            .with_exposed_port(3000),
    );
    let app_container = docker.run(app_image);

    // Get the mapped port for the app
    let app_port = 3000;
    let host_port = app_container.get_host_port_ipv4(app_port);

    // Now you can send HTTP requests to your app at localhost:app_port
    // e.g., use reqwest to test endpoints
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://localhost:{}/v1/health", host_port))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
}
