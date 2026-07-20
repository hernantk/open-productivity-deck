use crate::{
    audio::{self, AudioState},
    state::{AppState, RemoteState},
    tls,
};
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{sse::{Event, KeepAlive, Sse}, Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use axum_server::tls_rustls::RustlsConfig;
use serde::Deserialize;
use std::{convert::Infallible, net::{IpAddr, SocketAddr}, time::Duration};
use uuid::Uuid;

const MOBILE_HTML: &str = include_str!("../mobile/index.html");
const SETUP_HTML: &str = include_str!("../mobile/setup.html");
const MANIFEST: &str = include_str!("../mobile/manifest.webmanifest");
const SERVICE_WORKER: &str = include_str!("../mobile/sw.js");
const APP_ICON: &[u8] = include_bytes!("../icons/icon.png");
type ApiError = (StatusCode, String);

#[derive(Clone)]
struct BootstrapState {
    ca_der: Bytes,
    local_address: String,
    port: u16,
    secure_port: u16,
}

#[derive(Deserialize)]
struct AuthQuery {
    token: String,
}

#[derive(Deserialize)]
struct VolumeRequest {
    value: f32,
}

pub async fn run(state: AppState) -> Result<(), String> {
    let server_ip: IpAddr = state.local_address.parse().map_err(|error| format!("Endereço local inválido: {error}"))?;
    let material = tls::create(&state.tls_dir, server_ip)?;
    let tls_config = RustlsConfig::from_der(
        vec![material.server_certificate_der],
        material.server_key_der,
    )
    .await
    .map_err(|error| format!("Não foi possível configurar o HTTPS: {error}"))?;

    let bootstrap_state = BootstrapState {
        ca_der: Bytes::from(material.ca_der),
        local_address: state.local_address.clone(),
        port: state.port,
        secure_port: state.secure_port,
    };
    let http_router = Router::new()
        .route("/", get(setup))
        .route("/setup", get(setup))
        .route("/ca.cer", get(download_ca))
        .route("/health", get(http_health))
        .with_state(bootstrap_state);
    let https_router = Router::new()
        .route("/", get(index))
        .route("/manifest.webmanifest", get(manifest))
        .route("/sw.js", get(service_worker))
        .route("/icons/icon-192.png", get(app_icon))
        .route("/icons/icon-512.png", get(app_icon))
        .route("/apple-touch-icon.png", get(app_icon))
        .route("/health", get(health))
        .route("/api/state", get(api_state))
        .route("/api/events", get(unread_events))
        .route("/api/volume", post(set_volume))
        .route("/api/mute", post(toggle_mute))
        .route("/api/microphone", post(toggle_microphone))
        .route("/api/actions/{id}", post(launch_action))
        .with_state(state.clone());

    let http_listener = tokio::net::TcpListener::bind(("0.0.0.0", state.port))
        .await
        .map_err(|error| format!("A porta HTTP {} não está disponível: {error}", state.port))?;
    let secure_address = SocketAddr::from(([0, 0, 0, 0], state.secure_port));

    let http_server = async move {
        axum::serve(http_listener, http_router)
            .await
            .map_err(|error| format!("O servidor de instalação foi encerrado: {error}"))
    };
    let https_server = async move {
        axum_server::bind_rustls(secure_address, tls_config)
            .serve(https_router.into_make_service())
            .await
            .map_err(|error| format!("A PWA segura foi encerrada: {error}"))
    };

    tokio::try_join!(http_server, https_server)?;
    Ok(())
}

async fn setup(State(state): State<BootstrapState>) -> Html<String> {
    Html(SETUP_HTML.replace("__SECURE_PORT__", &state.secure_port.to_string()))
}

async fn download_ca(State(state): State<BootstrapState>) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/x-x509-ca-cert"));
    headers.insert(header::CONTENT_DISPOSITION, HeaderValue::from_static("attachment; filename=\"open-productivity-deck-ca.cer\""));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    (headers, state.ca_der)
}

async fn http_health(State(state): State<BootstrapState>) -> String {
    format!("ok http://{}:{} -> https://{}:{}", state.local_address, state.port, state.local_address, state.secure_port)
}

async fn index() -> Html<&'static str> {
    Html(MOBILE_HTML)
}

async fn manifest() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/manifest+json"), (header::CACHE_CONTROL, "no-cache")], MANIFEST)
}

async fn service_worker() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        SERVICE_WORKER,
    )
}

async fn app_icon() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "public, max-age=86400")], APP_ICON)
}

async fn health(State(state): State<AppState>) -> String {
    format!("ok https://{}:{}", state.local_address, state.secure_port)
}

async fn api_state(State(state): State<AppState>, Query(auth): Query<AuthQuery>) -> Result<Json<RemoteState>, ApiError> {
    authorize(&state, &auth)?;
    Ok(Json(state.remote()))
}

async fn unread_events(
    State(state): State<AppState>,
    Query(auth): Query<AuthQuery>,
) -> Result<impl IntoResponse, ApiError> {
    authorize(&state, &auth)?;
    let mut receiver = state.subscribe_unread();
    let stream_state = state.clone();
    let stream = async_stream::stream! {
        let initial = serde_json::to_string(&stream_state.unread_counts()).unwrap_or_else(|_| "{}".into());
        yield Ok::<Event, Infallible>(Event::default().event("unread").data(initial));
        loop {
            match receiver.recv().await {
                Ok(()) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let data = serde_json::to_string(&stream_state.unread_counts()).unwrap_or_else(|_| "{}".into());
                    yield Ok(Event::default().event("unread").data(data));
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)).text("keep-alive")))
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

async fn toggle_microphone(State(state): State<AppState>, Query(auth): Query<AuthQuery>) -> Result<Json<AudioState>, ApiError> {
    authorize(&state, &auth)?;
    audio::toggle_input_mute().map(Json).map_err(internal_error)
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
