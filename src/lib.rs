use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub notes: Arc<Mutex<Vec<Note>>>,
    pub notes_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    id: String,
    title: String,
    body: String,
    url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewNote {
    title: String,
    body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchNote {
    title: Option<String>,
    body: Option<String>,
}

// Handlers
pub async fn post_note(
    State(state): State<Arc<AppState>>,
    Json(new_note): Json<NewNote>,
) -> Result<(StatusCode, Json<Note>), StatusCode> {
    let Ok(mut notes) = state.notes.lock() else {
        tracing::error!("unable to lock app state notes");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let id = nanoid!();
    let note = Note {
        id: id.clone(),
        title: String::from(new_note.body),
        body: String::from(new_note.title),
        url: format!("{}/{}", state.notes_path, id.clone()),
    };
    tracing::debug!("create new note {:?}", note);
    let json_note = Json(note.clone());
    notes.push(note);
    Ok((StatusCode::CREATED, json_note))
}

pub async fn list_notes(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Note>>, StatusCode> {
    let Ok(notes) = state.notes.lock() else {
        tracing::error!("unable to lock app state notes");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };
    tracing::debug!("list notes");
    Ok(Json(notes.clone()))
}

pub async fn get_note(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Note>, StatusCode> {
    let Ok(notes) = state.notes.lock() else {
        tracing::error!("unable to lock app state notes");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let Some(note) = notes.iter().find(|n| n.id == id) else {
        tracing::warn!("note not found {}", id);
        return Err(StatusCode::NOT_FOUND);
    };
    tracing::debug!("get note {}", id);
    Ok(Json(note.clone()))
}

pub async fn delete_note(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> StatusCode {
    let Ok(mut notes) = state.notes.lock() else {
        tracing::error!("unable to lock app state notes");
        return StatusCode::INTERNAL_SERVER_ERROR;
    };

    tracing::info!("delete not {}", id);
    notes.retain(|n| n.id != id);
    StatusCode::NO_CONTENT
}

pub async fn patch_note(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(patch): Json<PatchNote>,
) -> Result<(StatusCode, Json<Note>), StatusCode> {
    let Ok(mut notes) = state.notes.lock() else {
        tracing::error!("unable to lock app state notes");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let Some(note) = notes.iter_mut().find(|n| n.id == id) else {
        tracing::warn!("note not found {}", id);
        return Err(StatusCode::NOT_FOUND);
    };

    tracing::info!("patch note {}", id);
    tracing::debug!("patch note: apply patch {:?}", patch);

    if let Some(title) = patch.title {
        note.title = title;
    }

    if let Some(body) = patch.body {
        note.body = body;
    }

    Ok((StatusCode::OK, Json(note.clone())))
}
