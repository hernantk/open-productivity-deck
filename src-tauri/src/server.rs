use crate::{
    audio::{self, AudioState},
    state::{AppState, RemoteState},
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Html,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

const MOBILE_HTML: &str = include_str!("../mobile/index.html");
type ApiError = (StatusCode, String);

#[derive(Deserialize)]
struct AuthQuery {
    token: String,
}

#[derive(Deserialize)]
struct VolumeRequest {
    value: f32,
}

pub async fn run(state: AppState) -> Result<(), String> {
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", state.port))
        .await
        .map_err(|error| format!("A porta {} não está disponível: {error}", state.port))?;

    let router = Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .route("/api/state", get(api_state))
        .route("/api/volume", post(set_volume))
        .route("/api/mute", post(toggle_mute))
        .route("/api/actions/{id}", post(launch_action))
        .with_state(state);

    axum::serve(listener, router)
        .await
        .map_err(|error| format!("O servidor local foi encerrado: {error}"))
}

async fn index() -> Html<&'static str> {
    Html(MOBILE_HTML)
}

async fn health(State(state): State<AppState>) -> String {
    format!("ok {}:{}", state.local_address, state.port)
}

async fn api_state(State(state): State<AppState>, Query(auth): Query<AuthQuery>) -> Result<Json<RemoteState>, ApiError> {
    authorize(&state, &auth)?;
    Ok(Json(state.remote()))
}

async fn set_volume(
    State(state): State<AppState>,
    Query(auth): Query<AuthQuery>,
    Json(request): Json<VolumeRequest>,
) -> Result<Json<AudioState>, ApiError> {
    authorize(&state, &auth)?;
    audio::set_volume(request.value).map(Json).map_err(internal_error)
}

async fn toggle_mute(State(state): State<AppState>, Query(auth): Query<AuthQuery>) -> Result<Json<AudioState>, ApiError> {
    authorize(&state, &auth)?;
    audio::toggle_mute().map(Json).map_err(internal_error)
}

async fn launch_action(
    State(state): State<AppState>,
    Query(auth): Query<AuthQuery>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    authorize(&state, &auth)?;
    state.launch(id).map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
}

fn authorize(state: &AppState, auth: &AuthQuery) -> Result<(), ApiError> {
    if state.authorize(&auth.token) {
        Ok(())
    } else {
        Err((StatusCode::UNAUTHORIZED, "Este acesso expirou. Leia o QR Code novamente.".into()))
    }
}

fn internal_error(message: String) -> ApiError {
    (StatusCode::INTERNAL_SERVER_ERROR, message)
}
