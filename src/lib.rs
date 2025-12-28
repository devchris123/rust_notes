use std::sync::Arc;
use tokio::sync::Mutex;

use tracing_subscriber::{self, layer::SubscriberExt, util::SubscriberInitExt};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};

use nanoid::nanoid;
use tower_http::trace::TraceLayer;

pub mod notes;
pub mod persistency;

use notes::*;

use crate::persistency::{create_mongo_client, NoteMongoDb};

const APP_NAME: &str = "notes";

pub struct AppConfig {
    pub host_port: String,
    pub api_version: String,
}

pub struct AppState {
    pub notes: Arc<Mutex<dyn NoteDb + Send + Sync>>,
    pub notes_path: String,
}

pub async fn create_app(
    app_config: AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // Setup tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from(format!(
            "RUST_LOG={},{}=debug,tower_http=debug,axum::rejection=trace",
            std::env::var("RUST_LOG").unwrap_or("info".to_string()),
            env!("CARGO_CRATE_NAME")
        )))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Setup server address
    let notes_path =
        format!("{}/{}/notes", app_config.host_port, app_config.api_version);

    // Setup notes DB
    let uri = "uri";
    let client = create_mongo_client(uri).await;
    let Ok(client) = client else {
        tracing::error!("unable to get database client");
        return Err(client.unwrap_err().into());
    };
    let db = NoteMongoDb::get_notes_db(client);
    let note_db = NoteMongoDb::new(db);

    let state = Arc::new(AppState {
        notes: Arc::new(Mutex::new(note_db)),
        notes_path,
    });

    let app = create_axum_app(state, &app_config.api_version);

    // Setup TCP listener
    let span = tracing::info_span!(
        "Start app",
        app = APP_NAME,
        api_version = app_config.api_version
    );
    let _enter = span.enter();
    tracing::debug!("Setup listener on {}", app_config.host_port);
    let listener = match tokio::net::TcpListener::bind(&app_config.host_port)
        .await
    {
        Ok(listener) => listener,
        Err(err) => {
            tracing::error!("unable to setup lister {}", app_config.host_port);
            return Err(err.into());
        }
    };

    // Setup listening
    tracing::info!("Serve on {}", app_config.host_port);
    if let Err(err) = axum::serve(listener, app).await {
        tracing::error!(
            "unable to serve app for listener at {}",
            app_config.host_port
        );
        return Err(err.into());
    }
    Ok(())
}

fn create_axum_app(state: Arc<AppState>, api_version: &str) -> Router {
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
    let notes = state.notes.lock().await;
    let id = nanoid!();
    let note = Note {
        id: id.clone(),
        title: String::from(new_note.title),
        body: String::from(new_note.body),
        url: format!("{}/{}", state.notes_path, id.clone()),
    };
    tracing::debug!("create new note {:?}", note);
    let Ok(_) = notes.create_note(&note).await else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };
    let Ok(note) = notes.get_note(&id).await else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };
    let Some(note) = note else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };
    Ok((StatusCode::CREATED, Json(note.clone())))
}

pub async fn list_notes(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Note>>, StatusCode> {
    let notes = state.notes.lock().await;
    tracing::debug!("list notes");
    let Ok(notes) = notes.list_notes().await else {
        tracing::error!("unable to get notes");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };
    Ok(Json(notes))
}

pub async fn get_note(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Note>, StatusCode> {
    let notes = state.notes.lock().await;
    let note = notes.get_note(&id).await;
    let Ok(note) = note else {
        tracing::error!("unable to get note");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };
    let Some(note) = note else {
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
    let notes = state.notes.lock().await;
    tracing::info!("delete note {}", id);
    let Ok(res) = notes.delete_note(&id).await else {
        tracing::error!("unable to delete note {}", id);
        return StatusCode::INTERNAL_SERVER_ERROR;
    };

    if !res {
        tracing::info!("unable to delete note {} (not found)", id);
        return StatusCode::NOT_FOUND;
    }

    StatusCode::NO_CONTENT
}

pub async fn patch_note(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(patch): Json<PatchNote>,
) -> Result<(StatusCode, Json<Note>), StatusCode> {
    let notes = state.notes.lock().await;

    tracing::info!("patch note {}", id);
    tracing::debug!("patch note: apply patch {:?}", patch);

    let res = notes.update_note(&id, &patch).await;

    let Ok(()) = res else {
        tracing::error!("unable to update note");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let Ok(note) = notes.get_note(&id).await else {
        tracing::error!("unable to get note after update");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let Some(note) = note else {
        tracing::error!("unable to get note after update");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    Ok((StatusCode::OK, Json(note.clone())))
}

#[cfg(test)]
mod tests {
    use super::*;

    use async_trait::async_trait;
    use axum::{body::Body, http::Request, response::Response};
    use http_body_util::BodyExt;
    use std::sync::{
        self,
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use tower::ServiceExt;

    impl NewNote {
        fn new(title: &str, body: &str) -> NewNote {
            NewNote {
                title: title.to_string(),
                body: body.to_string(),
            }
        }
    }

    struct NoteVecDb {
        vec: sync::Mutex<Vec<Note>>,
        fail_create: AtomicBool,
        fail_get: AtomicBool,
        none_get: AtomicBool,
        fail_update: AtomicBool,
        fail_delete: AtomicBool,
        fail_list: AtomicBool,
    }

    impl NoteVecDb {
        pub fn new(vec: sync::Mutex<Vec<Note>>) -> NoteVecDb {
            NoteVecDb {
                vec,
                fail_create: AtomicBool::new(false),
                fail_get: AtomicBool::new(false),
                none_get: AtomicBool::new(false),
                fail_delete: AtomicBool::new(false),
                fail_list: AtomicBool::new(false),
                fail_update: AtomicBool::new(false),
            }
        }

        pub fn set_fail_create(&self, value: bool) {
            self.fail_create
                .store(value, sync::atomic::Ordering::SeqCst);
        }
        pub fn set_fail_get(&self, value: bool) {
            self.fail_get.store(value, sync::atomic::Ordering::SeqCst);
        }
        pub fn set_none_get(&self, value: bool) {
            self.fail_get.store(value, sync::atomic::Ordering::SeqCst);
        }
        pub fn set_fail_update(&self, value: bool) {
            self.fail_update
                .store(value, sync::atomic::Ordering::SeqCst);
        }
        pub fn set_fail_delete(&self, value: bool) {
            self.fail_delete
                .store(value, sync::atomic::Ordering::SeqCst);
        }
        pub fn set_fail_list(&self, value: bool) {
            self.fail_list.store(value, sync::atomic::Ordering::SeqCst);
        }
    }

    #[async_trait]
    impl NoteDb for NoteVecDb {
        async fn create_note(
            &self,
            note: &Note,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            if self.fail_create.load(Ordering::SeqCst) {
                return Err("simulated create error".into());
            }
            self.vec.lock().unwrap().push(note.clone());
            Ok(())
        }

        async fn get_note(
            &self,
            id: &str,
        ) -> Result<Option<Note>, Box<dyn std::error::Error + Send + Sync>>
        {
            if self.fail_get.load(Ordering::SeqCst) {
                return Err("simulated get error".into());
            }
            if self.none_get.load(Ordering::SeqCst) {
                return Ok(None);
            }
            let vec = self.vec.lock().unwrap();
            let Some(note) = vec.iter().find(|n| n.id == id) else {
                return Ok(None);
            };
            return Ok(Some(note.clone()));
        }

        async fn update_note(
            &self,
            id: &str,
            note: &PatchNote,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            if self.fail_update.load(Ordering::SeqCst) {
                return Err("simulated get error".into());
            }
            let mut vec = self.vec.lock().unwrap();
            let Some(get_note) = vec.iter_mut().find(|n| n.id == id) else {
                return Ok(());
            };
            if let Some(title) = &note.title {
                get_note.title = title.to_string();
            }

            if let Some(body) = &note.body {
                get_note.body = body.to_string();
            }
            Ok(())
        }

        async fn delete_note(
            &self,
            id: &str,
        ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
            if self.fail_delete.load(Ordering::SeqCst) {
                return Err("simulated get error".into());
            }
            let mut vec = self.vec.lock().unwrap();
            let Some(_) = vec.iter().find(|n| n.id == id) else {
                return Ok(false);
            };
            vec.retain(|n| n.id != id);
            Ok(true)
        }

        async fn list_notes(
            &self,
        ) -> Result<Vec<Note>, Box<dyn std::error::Error + Send + Sync>>
        {
            if self.fail_list.load(Ordering::SeqCst) {
                return Err("simulated get error".into());
            }
            Ok(self.vec.lock().unwrap().clone())
        }
    }

    #[tokio::test]
    async fn it_fails_to_create_a_note() {
        // Setup
        let (app, state) = create_test_app();
        state.lock().await.set_fail_create(true);
        let new_note = NewNote {
            title: "a".to_string(),
            body: "b".to_string(),
        };

        // Execute
        let resp = post_test_note(app, new_note).await;

        // Assert
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn it_fails_to_get_a_note_after_creating() {
        // Setup
        let (app, state) = create_test_app();
        state.lock().await.set_fail_get(true);
        let new_note = NewNote {
            title: "a".to_string(),
            body: "b".to_string(),
        };

        // Execute
        let resp = post_test_note(app, new_note).await;

        // Assert
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn it_gets_none_after_creating() {
        // Setup
        let (app, state) = create_test_app();
        state.lock().await.set_none_get(true);
        let new_note = NewNote {
            title: "a".to_string(),
            body: "b".to_string(),
        };

        // Execute
        let resp = post_test_note(app, new_note).await;

        // Assert
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn it_fails_to_update_a_note() {
        // Setup
        let (app, state) = create_test_app();
        state.lock().await.set_fail_update(true);
        let new_note = NewNote {
            title: "a".to_string(),
            body: "b".to_string(),
        };
        let resp = post_test_note(app.clone(), new_note).await;
        let note = deserialize_note(resp.into_body()).await;

        // Execute
        let resp = patch_test_note(
            app,
            &note.id,
            PatchNote {
                title: None,
                body: None,
            },
        )
        .await;

        // Assert
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn it_fails_to_delete_a_note() {
        // Setup
        let (app, state) = create_test_app();
        state.lock().await.set_fail_delete(true);
        let new_note = NewNote {
            title: "a".to_string(),
            body: "b".to_string(),
        };
        let resp = post_test_note(app.clone(), new_note).await;
        let note = deserialize_note(resp.into_body()).await;

        // Execute
        let resp = delete_test_note(app, &note.id).await;

        // Assert
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn it_fails_to_delete_a_note_not_found() {
        // Setup
        let (app, _) = create_test_app();

        // Execute
        let resp = delete_test_note(app, &nanoid!()).await;

        // Assert
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn it_fails_to_list_notes() {
        // Setup
        let (app, state) = create_test_app();
        state.lock().await.set_fail_list(true);

        // Execute
        let resp = list_test_notes(app).await;

        // Assert
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn it_creates_a_note() {
        // Setup
        let (app, _) = create_test_app();
        let new_note = NewNote {
            title: "a".to_string(),
            body: "b".to_string(),
        };

        // Execute
        let resp = post_test_note(app, new_note).await;

        // Assert
        assert_eq!(resp.status(), StatusCode::CREATED);
        let note_json = deserialize_note(resp.into_body()).await;
        assert_eq!(note_json.title, "a");
        assert_eq!(note_json.body, "b");
    }

    #[tokio::test]
    async fn it_gets_a_note() {
        // Setup
        let (app, _) = create_test_app();
        let new_note = NewNote {
            title: "a".to_string(),
            body: "b".to_string(),
        };
        let resp = post_test_note(app.clone(), new_note).await;
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
        let (app, _) = create_test_app();
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
        let (app, _) = create_test_app();
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
        let (app, _) = create_test_app();
        let note = NewNote::new("note0", "body0");
        let note = post_test_note(app.clone(), note).await;
        let note = deserialize_note(note.into_body()).await;

        // Execute
        let resp = patch_test_note(
            app.clone(),
            &note.id,
            PatchNote {
                title: Some("newtitle".to_string()),
                body: Some("newbody".to_string()),
            },
        )
        .await;

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

    fn create_test_app() -> (axum::Router, Arc<Mutex<NoteVecDb>>) {
        let notes = Vec::<Note>::new();
        let notes_path = "/notes";
        let notes =
            Arc::new(Mutex::new(NoteVecDb::new(sync::Mutex::new(notes))));
        let state = Arc::new(AppState {
            notes: notes.clone(),
            notes_path: notes_path.to_string(),
        });
        (create_axum_app(state.clone(), "v1"), notes)
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

    async fn patch_test_note(
        app: axum::routing::Router,
        id: &str,
        note: PatchNote,
    ) -> Response<Body> {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/v1/notes/{}", id))
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_string(&note).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn delete_test_note(
        app: axum::routing::Router,
        id: &str,
    ) -> Response<Body> {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/v1/notes/{}", id))
                    .header("Content-Type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn list_test_notes(app: axum::routing::Router) -> Response<Body> {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/notes")
                    .header("Content-Type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }
}
