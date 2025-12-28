use async_trait::async_trait;
use nanoid::nanoid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub body: String,
    pub url: String,
}

impl Note {
    pub fn new(title: &str, body: &str, url: &str) -> Note {
        let id = nanoid!();
        Note {
            id: id.clone(),
            title: title.to_string(),
            body: body.to_string(),
            url: url.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewNote {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchNote {
    pub title: Option<String>,
    pub body: Option<String>,
}

#[async_trait]
pub trait NoteDb: Send + Sync {
    async fn create_note(
        &self,
        note: &Note,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    async fn get_note(
        &self,
        id: &str,
    ) -> Result<Option<Note>, Box<dyn std::error::Error + Send + Sync>>;

    async fn update_note(
        &self,
        id: &str,
        note: &PatchNote,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    async fn delete_note(
        &self,
        id: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    async fn list_notes(
        &self,
    ) -> Result<Vec<Note>, Box<dyn std::error::Error + Send + Sync>>;
}
