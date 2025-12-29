use mongodb::{options::ClientOptions, Client};
use testcontainers::{clients, GenericImage, RunnableImage};

use notes::{
    notes::{Note, NoteDb, PatchNote},
    persistency::NoteMongoDb,
};

#[tokio::test]
async fn test_with_mongodb_container() {
    // Start Docker client
    let docker = clients::Cli::default();

    // Start MongoDB container
    let mongo_image = RunnableImage::from(
        GenericImage::new("mongo", "7.0.5") // Use a stable MongoDB version
            .with_exposed_port(27017),
    );
    let node = docker.run(mongo_image);

    // Get the port that Docker mapped
    let port = node.get_host_port_ipv4(27017);

    // Build the MongoDB connection string
    let uri = format!("mongodb://localhost:{}", port);

    // Connect to MongoDB
    let options = ClientOptions::parse(&uri).await.unwrap();
    let client = Client::with_options(options).unwrap();

    // Get notes DB
    let db = NoteMongoDb::get_notes_db(client);
    let note_db = NoteMongoDb::new(db);

    let create_note = Note::new("note", "body", "url");
    note_db.create_note(&create_note).await.unwrap();
    let get_note = note_db.get_note(&create_note.id).await.unwrap();
    match get_note {
        Some(note) => assert_eq!(note.id, create_note.id),
        None => panic!("expected note"),
    }
    let patch_note = PatchNote {
        title: Some("newtitle".to_string()),
        body: Some("newbody".to_string()),
    };
    note_db
        .update_note(&create_note.id, &patch_note)
        .await
        .unwrap();
    let get_note = note_db.get_note(&create_note.id).await.unwrap();
    match get_note {
        Some(note) => {
            assert_eq!(note.id, create_note.id);
            assert_eq!(note.title, patch_note.title.unwrap());
            assert_eq!(note.body, patch_note.body.unwrap());
        }
        None => panic!("expected note"),
    }
    let deleted = note_db.delete_note(&create_note.id).await.unwrap();
    assert!(deleted);
    let get_note = note_db.get_note(&create_note.id).await.unwrap();
    if get_note.is_some() {
        panic!("expected no note");
    };
}
