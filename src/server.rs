use argh::FromArgs;
use axum::{Json, Router, extract::State, routing::post};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, postgres::PgPoolOptions};

#[derive(FromArgs, PartialEq, Debug)]
/// Serve the server
#[argh(subcommand, name = "serve")]
pub struct ServeCommand {
    #[argh(positional)]
    /// config file
    pub config: String,
}

impl ServeCommand {
    pub fn run(&self) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(serve());
    }
}

#[derive(Clone)]
pub struct AppState {
    db: PgPool,
}

async fn serve() {
    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL not set"))
        .await
        .expect("failed to connect to postgres");

    sqlx::migrate!("./migrations").run(&db).await.unwrap();

    let state = AppState { db };

    let app = Router::new()
        .route("/emit", post(emit_state))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();

    log::info!("🌱 Server listening on :8080");
    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StateMessage {
    kind: MsgKind,
    hostname: String,
    fingerprint: String,
    payload: String,
    signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MsgKind {}

pub async fn emit_state(
    State(app): State<AppState>,
    body: Json<StateMessage>,
) -> Result<StatusCode, StatusCode> {
    // Deserialize state message
    // Verify Signature
    // Handle state message

    log::info!("✍️ Got state");

    Ok(StatusCode::OK)
}
