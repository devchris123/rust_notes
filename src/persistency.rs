use async_trait::async_trait;
use mongodb::{bson::doc, options::ClientOptions, Client, Database};

use crate::notes::{Note, NoteDb, PatchNote};

use futures::stream::TryStreamExt;

const NOTES_DB: &str = "notes";
const NOTES_COLLECTION: &str = "notes";

pub async fn create_mongo_client(
    uri: &str,
) -> Result<Client, mongodb::error::Error> {
    let options = ClientOptions::parse(uri).await?;
    let client = Client::with_options(options)?;
    Ok(client)
}

pub struct NoteMongoDb {
    db: Database,
}

impl NoteMongoDb {
    pub fn get_notes_db(client: Client) -> Database {
        client.database(NOTES_DB)
    }

    pub fn new(db: Database) -> NoteMongoDb {
        NoteMongoDb { db }
    }
}

#[async_trait]
impl NoteDb for NoteMongoDb {
    async fn create_note(
        &self,
        note: &Note,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let coll = self.db.collection::<Note>(NOTES_COLLECTION);
        coll.insert_one(note).await?;
        Ok(())
    }

    async fn get_note(
        &self,
        id: &str,
    ) -> Result<Option<Note>, Box<dyn std::error::Error + Send + Sync>> {
        let coll = self.db.collection::<Note>(NOTES_COLLECTION);
        let option = coll.find_one(doc! { "id": id }).await?;
        Ok(option)
    }

    async fn update_note(
        &self,
        id: &str,
        note: &PatchNote,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let coll = self.db.collection::<Note>(NOTES_COLLECTION);
        let filter = doc! { "id": id };
        let update = doc! {
            "$set": {
                "title": &note.title,
                "body": &note.body
            }
        };
        coll.update_one(filter, update).await?;
        Ok(())
    }

    async fn delete_note(
        &self,
        id: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let coll = self.db.collection::<Note>(NOTES_COLLECTION);
        let filter = doc! { "id": id };
        let res = coll.delete_one(filter).await?;
        Ok(res.deleted_count > 0)
    }

    async fn list_notes(
        &self,
    ) -> Result<Vec<Note>, Box<dyn std::error::Error + Send + Sync>> {
        let coll = self.db.collection::<Note>(NOTES_COLLECTION);
        let mut cursor = coll.find(doc! {}).await?;
        let mut notes = Vec::new();
        while let Some(note) = cursor.try_next().await? {
            notes.push(note);
        }
        Ok(notes)
    }
}
