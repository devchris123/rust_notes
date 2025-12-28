use std::sync::{Arc, Mutex};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};

use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;

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

pub fn create_app(state: Arc<AppState>, api_version: &str) -> Router {
    Router::new()
        .route(
            &format!("/{}/notes", api_version),
            post(post_note).get(list_notes),
        )
        .route(
            &format!("/{}/notes/{{id}}", api_version),
            get(get_note).delete(delete_note).patch(patch_note),
        )
        .with_state(state)
        .layer(TraceLayer::new_for_http())
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
        title: String::from(new_note.title),
        body: String::from(new_note.body),
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

#[cfg(test)]
mod tests {
    use super::*;

    use axum::{body::Body, http::Request, response::Response};
    use http_body_util::BodyExt; // for .collect()
    use tower::ServiceExt;

    impl NewNote {
        fn new(title: &str, body: &str) -> NewNote {
            NewNote {
                title: title.to_string(),
                body: body.to_string(),
            }
        }
    }

    #[tokio::test]
    async fn it_creates_a_note() {
        // Setup
        let app = create_test_app();
        let new_note = NewNote {
            title: "a".to_string(),
            body: "b".to_string(),
        };

        // Execute
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/notes")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_string(&new_note).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Assert
        assert_eq!(resp.status(), StatusCode::CREATED);
        let note_json = deserialize_note(resp.into_body()).await;
        assert_eq!(note_json.title, "a");
        assert_eq!(note_json.body, "b");
    }

    #[tokio::test]
    async fn it_gets_a_note() {
        // Setup
        let app = create_test_app();
        let new_note = NewNote {
            title: "a".to_string(),
            body: "b".to_string(),
        };
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/notes")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_string(&new_note).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let note_json = deserialize_note(resp.into_body()).await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/notes/{}", note_json.id))
                    .header("Content-Type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Assert
        assert_eq!(resp.status(), StatusCode::OK);
        let note_json = deserialize_note(resp.into_body()).await;
        assert_eq!(note_json.title, "a");
        assert_eq!(note_json.body, "b");
    }

    #[tokio::test]
    async fn it_lists_notes() {
        // Setup
        let app = create_test_app();
        let note = NewNote::new("note0", "body0");
        let _ = post_test_note(app.clone(), note).await;
        let note = NewNote::new("note01", "body1");
        let _ = post_test_note(app.clone(), note).await;

        // Execute
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/notes")
                    .header("Content-Type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Assert
        assert_eq!(resp.status(), StatusCode::OK);
        let notes: Vec<Note> = deserialize_notes(resp.into_body()).await;
        assert_eq!(notes.len(), 2);
    }

    #[tokio::test]
    async fn it_deletes_a_note() {
        // Setup
        let app = create_test_app();
        let note = NewNote::new("note0", "body0");
        let note0 = post_test_note(app.clone(), note).await;
        let note0 = deserialize_note(note0.into_body()).await;
        let note = NewNote::new("note01", "body1");
        let note = post_test_note(app.clone(), note).await;
        let note = deserialize_note(note.into_body()).await;

        // Execute
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/v1/notes/{}", note.id))
                    .header("Content-Type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Assert
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/notes")
                    .header("Content-Type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let notes = deserialize_notes(resp.into_body()).await;
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, note0.id);
    }

    #[tokio::test]
    async fn it_patches_a_note() {
        // Setup
        let app = create_test_app();
        let note = NewNote::new("note0", "body0");
        let note = post_test_note(app.clone(), note).await;
        let mut note = deserialize_note(note.into_body()).await;
        note.title = "newtitle".to_string();
        note.body = "newbody".to_string();

        // Execute
        let resp = app
            .clone()
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/v1/notes/{}", note.id))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_string(&note).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // assert
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/notes/{}", note.id))
                    .header("Content-Type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let patched_noted = deserialize_note(resp.into_body()).await;
        assert_eq!(patched_noted.id, note.id);
        assert_eq!(patched_noted.title, "newtitle");
        assert_eq!(patched_noted.body, "newbody");
    }

    fn create_test_app() -> axum::Router {
        let notes = Arc::new(Mutex::new(Vec::<Note>::new()));
        let notes_path = "/notes";
        let state = Arc::new(AppState {
            notes,
            notes_path: notes_path.to_string(),
        });
        create_app(state, "v1")
    }

    async fn deserialize_note(body: axum::body::Body) -> Note {
        let note_bytes = body.collect().await.unwrap().to_bytes();
        serde_json::from_slice::<Note>(&note_bytes).unwrap()
    }

    async fn deserialize_notes(body: axum::body::Body) -> Vec<Note> {
        let note_bytes = body.collect().await.unwrap().to_bytes();
        serde_json::from_slice::<Vec<Note>>(&note_bytes).unwrap()
    }

    async fn post_test_note(
        app: axum::routing::Router,
        new_note: NewNote,
    ) -> Response<Body> {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/notes")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_string(&new_note).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }
}
