use argh::FromArgs;
use axum::{Json, Router, extract::State, routing::post};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, postgres::PgPoolOptions};

use crate::restic::{BackupEmitSummary, ResticSummaryMsg, hostname, machine_id};

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
    pub kind: MsgKind,
    pub hostname: String,
    pub fingerprint: String,
    pub payload: String,
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MsgKind {
    Backup,
}

pub async fn emit_state(
    State(app): State<AppState>,
    body: Json<StateMessage>,
) -> Result<StatusCode, StatusCode> {
    // Verify Signature
    log::info!("✍️ Got state");

    match body.kind {
        MsgKind::Backup => {
            let x: BackupEmitSummary = facet_json::from_str(&body.payload).unwrap();
            persist_summary_msg(
                &app.db,
                x.summary.unwrap(),
                &x.src.join(";"),
                &x.target,
                &x.status,
            )
            .await
        }
    }

    Ok(StatusCode::OK)
}

pub async fn persist_summary_msg(
    pool: &PgPool,
    msg: ResticSummaryMsg,
    src: &str,
    target: &str,
    status: &str,
) {
    sqlx::query(
        r#"
           INSERT INTO restic_summary_msg (
               changed_snapshots,
               files_new,
               files_changed,
               files_unmodified,
               dirs_new,
               dirs_changed,
               dirs_unmodified,
               data_blobs,
               tree_blobs,
               data_added,
               data_added_packed,
               total_files_processed,
               total_bytes_processed,
               total_duration,
               backup_start,
               backup_end,
               snapshot_id,
               hostname,
               sshid,
               src,
               target,
               status
           )
           VALUES (
               $1, $2, $3, $4, $5, $6, $7, $8, $9,
               $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22
           )
           "#,
    )
    .bind(msg.changed_snapshots)
    .bind(msg.files_new)
    .bind(msg.files_changed)
    .bind(msg.files_unmodified)
    .bind(msg.dirs_new)
    .bind(msg.dirs_changed)
    .bind(msg.dirs_unmodified)
    .bind(msg.data_blobs)
    .bind(msg.tree_blobs)
    .bind(msg.data_added)
    .bind(msg.data_added_packed)
    .bind(msg.total_files_processed)
    .bind(msg.total_bytes_processed)
    .bind(msg.total_duration)
    .bind(msg.backup_start)
    .bind(msg.backup_end)
    .bind(msg.snapshot_id)
    .bind(hostname())
    .bind(machine_id())
    .bind(src)
    .bind(target)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
}
