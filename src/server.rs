use argh::FromArgs;
use axum::{Json, Router, extract::{Path, State}, routing::{get, post}};
use maud::{DOCTYPE, Markup, html};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, postgres::PgPoolOptions};

use crate::restic::{BackupEmitSummary, ResticSummaryMsg};

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
        .route("/", get(ui_index))
        .route("/events", get(ui_events))
        .route("/hosts/{hostname}", get(ui_host))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();

    log::info!("🌱 Server listening on :8080");
    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StateMessage {
    pub kind: MsgKind,
    pub hostname: String,
    /// SHA256 fingerprint of the SSH host key (informational)
    pub fingerprint: String,
    /// Full OpenSSH public key used to verify the signature
    pub public_key: String,
    pub payload: String,
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MsgKind {
    Backup,
    Snapshots,
    Forget,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ForgetEmitSummary {
    pub target: String,
    pub removed: usize,
    pub kept: usize,
    pub dry_run: bool,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotEntry {
    pub id: String,
    pub short_id: String,
    pub hostname: String,
    pub paths: Vec<String>,
    pub tags: Vec<String>,
    pub time: String,
    pub username: Option<String>,
}

pub async fn emit_state(
    State(app): State<AppState>,
    body: Json<StateMessage>,
) -> Result<StatusCode, StatusCode> {
    log::info!("✍️  Got state from {}", body.hostname);

    // Look up the stored public key for this host (TOFU: trust on first use).
    let stored_key: Option<String> = sqlx::query_scalar(
        "SELECT public_key FROM known_hosts WHERE hostname = $1",
    )
    .bind(&body.hostname)
    .fetch_optional(&app.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let trusted_key = match stored_key {
        Some(k) => k,
        None => {
            // First time we've seen this host — register it implicitly.
            sqlx::query(
                "INSERT INTO known_hosts (hostname, public_key) VALUES ($1, $2)",
            )
            .bind(&body.hostname)
            .bind(&body.public_key)
            .execute(&app.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            log::info!("👋 Registered new host {} (fingerprint: {})", body.hostname, body.fingerprint);
            body.public_key.clone()
        }
    };

    // Verify the signature against the trusted (stored) key.
    let valid = crate::ssh::ssh_verify(
        &trusted_key,
        body.payload.as_bytes(),
        &body.signature,
    );
    if !valid {
        log::warn!(
            "🚨 Signature verification failed for {} (fingerprint: {})",
            body.hostname,
            body.fingerprint
        );
        return Err(StatusCode::UNAUTHORIZED);
    }
    log::info!("✅ Signature verified for {}", body.hostname);

    match body.kind {
        MsgKind::Backup => {
            let x: BackupEmitSummary = facet_json::from_str(&body.payload)
                .map_err(|_| StatusCode::BAD_REQUEST)?;
            persist_summary_msg(
                &app.db,
                x.summary,
                &x.src.join(";"),
                &x.target,
                &x.status,
                &body.hostname,
                &body.fingerprint,
            )
            .await?;
        }
        MsgKind::Forget => {
            let f: ForgetEmitSummary = serde_json::from_str(&body.payload)
                .map_err(|_| StatusCode::BAD_REQUEST)?;
            sqlx::query(
                "INSERT INTO forget_events (hostname, target, removed, kept, dry_run, status) \
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(&body.hostname)
            .bind(&f.target)
            .bind(f.removed as i64)
            .bind(f.kept as i64)
            .bind(f.dry_run)
            .bind(&f.status)
            .execute(&app.db)
            .await
            .map_err(|e| { log::error!("forget event insert failed: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
            log::info!("✂️  Forget from {}: {} removed, {} kept (dry={}, status={})",
                body.hostname, f.removed, f.kept, f.dry_run, f.status);
        }
        MsgKind::Snapshots => {
            let snaps: Vec<SnapshotEntry> = serde_json::from_str(&body.payload)
                .map_err(|_| StatusCode::BAD_REQUEST)?;
            for s in &snaps {
                sqlx::query(r#"
                    INSERT INTO snapshots (id, short_id, hostname, paths, tags, time, username)
                    VALUES ($1, $2, $3, $4, $5, $6::timestamptz, $7)
                    ON CONFLICT (id) DO UPDATE
                        SET short_id = EXCLUDED.short_id,
                            paths    = EXCLUDED.paths,
                            tags     = EXCLUDED.tags,
                            time     = EXCLUDED.time,
                            username = EXCLUDED.username
                "#)
                .bind(&s.id)
                .bind(&s.short_id)
                .bind(&body.hostname)
                .bind(s.paths.join(";"))
                .bind(s.tags.join(","))
                .bind(&s.time)
                .bind(&s.username)
                .execute(&app.db)
                .await
                .map_err(|e| { log::error!("snapshot upsert failed: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
            }
            log::info!("📸 Synced {} snapshots from {}", snaps.len(), body.hostname);
        }
    }

    Ok(StatusCode::OK)
}

const STALE_HOURS: i64 = 25;

fn host_class(last_status: Option<&str>, last_backup: Option<chrono::DateTime<chrono::Utc>>) -> &'static str {
    if last_status.map(|s| s.eq_ignore_ascii_case("error")).unwrap_or(false) {
        return "fail";
    }
    let stale = match last_backup {
        None => true,
        Some(t) => (chrono::Utc::now() - t).num_hours() >= STALE_HOURS,
    };
    if stale { "warn" } else { "ok" }
}

fn fmt_ago(t: chrono::DateTime<chrono::Utc>) -> String {
    let secs = (chrono::Utc::now() - t).num_seconds();
    if secs < 60 { format!("{}s ago", secs) }
    else if secs < 3600 { format!("{}m ago", secs / 60) }
    else if secs < 86400 { format!("{}h ago", secs / 3600) }
    else { format!("{}d ago", secs / 86400) }
}

fn fmt_bytes(n: i64) -> String {
    if n >= 1_073_741_824 {
        format!("{:.1} GiB", n as f64 / 1_073_741_824.0)
    } else if n >= 1_048_576 {
        format!("{:.1} MiB", n as f64 / 1_048_576.0)
    } else if n >= 1024 {
        format!("{:.1} KiB", n as f64 / 1024.0)
    } else {
        format!("{} B", n)
    }
}

fn page(title: &str, content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width,initial-scale=1";
                title { (title) }
                style {
                    r#"
:root{
  --bg:#0f1117;--surface:#1a1d27;--border:#2a2d3a;
  --text:#e2e4ed;--muted:#6b7280;
  --green:#22c55e;--green-dim:#14532d;
  --yellow:#eab308;--yellow-dim:#713f12;
  --red:#ef4444;--red-dim:#7f1d1d;
  --accent:#6366f1;
}
*{box-sizing:border-box;margin:0;padding:0}
body{background:var(--bg);color:var(--text);font-family:'SF Mono',ui-monospace,monospace;min-height:100vh}
header{padding:1.25rem 2rem;border-bottom:1px solid var(--border);display:flex;align-items:center;gap:2rem}
header h1{font-size:1.1rem;font-weight:600;letter-spacing:.05em;color:var(--accent)}
nav a{color:var(--muted);text-decoration:none;font-size:.85rem;padding:.3rem .6rem;border-radius:.3rem;transition:color .15s,background .15s}
nav a:hover{color:var(--text);background:var(--surface)}
nav a.active{color:var(--text)}
main{max-width:1200px;margin:0 auto;padding:2rem}
h2{font-size:.7rem;font-weight:600;letter-spacing:.12em;text-transform:uppercase;color:var(--muted);margin-bottom:1rem}
.cards{display:grid;grid-template-columns:repeat(auto-fill,minmax(260px,1fr));gap:1rem}
.card{background:var(--surface);border:1px solid var(--border);border-radius:.6rem;padding:1.25rem 1.4rem;display:flex;flex-direction:column;gap:.6rem;position:relative;overflow:hidden}
.card::before{content:'';position:absolute;top:0;left:0;right:0;height:3px}
.card.ok::before{background:var(--green)}
.card.warn::before{background:var(--yellow)}
.card.fail::before{background:var(--red)}
.card-host{font-size:1rem;font-weight:600;color:var(--text);display:flex;align-items:center;gap:.5rem}
.dot{width:8px;height:8px;border-radius:50%;flex-shrink:0}
.dot.ok{background:var(--green);box-shadow:0 0 6px var(--green)}
.dot.warn{background:var(--yellow);box-shadow:0 0 6px var(--yellow)}
.dot.fail{background:var(--red);box-shadow:0 0 6px var(--red)}
.card-meta{font-size:.78rem;color:var(--muted);display:flex;flex-direction:column;gap:.25rem}
.card-meta span{display:flex;gap:.4rem}
.card-meta .label{color:#4b5563}
.badge{display:inline-block;font-size:.7rem;font-weight:700;padding:.15rem .5rem;border-radius:.25rem;letter-spacing:.04em}
.badge.ok{background:var(--green-dim);color:var(--green)}
.badge.warn{background:var(--yellow-dim);color:var(--yellow)}
.badge.fail{background:var(--red-dim);color:var(--red)}
table{width:100%;border-collapse:collapse;font-size:.82rem}
thead tr{border-bottom:1px solid var(--border)}
th{color:var(--muted);font-weight:500;padding:.5rem .75rem;text-align:left;font-size:.72rem;letter-spacing:.08em;text-transform:uppercase}
td{padding:.55rem .75rem;border-bottom:1px solid var(--border);color:var(--text)}
tr:last-child td{border-bottom:none}
tr:hover td{background:var(--surface)}
.chip{display:inline-block;font-size:.7rem;font-weight:700;padding:.1rem .45rem;border-radius:.2rem}
.chip.ok{background:var(--green-dim);color:var(--green)}
.chip.warn{background:var(--yellow-dim);color:var(--yellow)}
.chip.fail{background:var(--red-dim);color:var(--red)}
a.card{text-decoration:none;cursor:pointer;transition:border-color .15s,transform .1s}
a.card:hover{border-color:#3f4255;transform:translateY(-1px)}
.stat-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(180px,1fr));gap:.75rem;margin-bottom:2rem}
.stat{background:var(--surface);border:1px solid var(--border);border-radius:.5rem;padding:1rem 1.2rem}
.stat-label{font-size:.65rem;font-weight:600;letter-spacing:.1em;text-transform:uppercase;color:var(--muted);margin-bottom:.35rem}
.stat-value{font-size:1.3rem;font-weight:700;color:var(--text)}
.stat-value.ok{color:var(--green)}
.stat-value.fail{color:var(--red)}
.section{margin-bottom:2.5rem}
.back{display:inline-flex;align-items:center;gap:.4rem;color:var(--muted);text-decoration:none;font-size:.8rem;margin-bottom:1.5rem}
.back:hover{color:var(--text)}
.host-title{display:flex;align-items:center;gap:.75rem;margin-bottom:1.75rem}
.host-title h1{font-size:1.4rem;font-weight:700;color:var(--text)}
code{font-family:inherit;background:var(--surface);border:1px solid var(--border);border-radius:.25rem;padding:.05rem .35rem;font-size:.8rem;color:var(--muted)}
                    "#
                }
            }
            body {
                header {
                    h1 { "bk" }
                    nav {
                        a href="/" { "Hosts" }
                        " "
                        a href="/events" { "Events" }
                    }
                }
                main { (content) }
            }
        }
    }
}

#[derive(sqlx::FromRow)]
struct HostSummaryRow {
    hostname: String,
    last_backup: Option<chrono::DateTime<chrono::Utc>>,
    last_status: Option<String>,
}

#[derive(sqlx::FromRow)]
struct EventRow {
    hostname: String,
    src: Option<String>,
    target: Option<String>,
    status: Option<String>,
    backup_end: Option<chrono::DateTime<chrono::Utc>>,
    total_bytes_processed: Option<i64>,
    files_new: Option<i64>,
    snapshot_id: Option<String>,
}

async fn ui_index(State(app): State<AppState>) -> Result<Markup, StatusCode> {
    let rows: Vec<HostSummaryRow> = sqlx::query_as(
        r#"
        SELECT
            hostname,
            MAX(backup_end) AS last_backup,
            (SELECT status FROM restic_summary_msg s2
             WHERE s2.hostname = s.hostname
             ORDER BY timestamp DESC LIMIT 1) AS last_status
        FROM restic_summary_msg s
        GROUP BY hostname
        ORDER BY last_backup DESC NULLS LAST
        "#,
    )
    .fetch_all(&app.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(page("bk", html! {
        h2 { "Hosts" }
        div.cards {
            @for r in &rows {
                @let cls = host_class(r.last_status.as_deref(), r.last_backup);
                @let status_label = r.last_status.as_deref().unwrap_or("unknown");
                a href=(format!("/hosts/{}", r.hostname)) class=(format!("card {cls}")) {
                    div.card-host {
                        div class=(format!("dot {cls}")) {}
                        span { (r.hostname) }
                        span style="margin-left:auto" {
                            span class=(format!("badge {cls}")) { (status_label.to_uppercase()) }
                        }
                    }
                    div.card-meta {
                        span {
                            span.label { "last backup" }
                            @if let Some(t) = r.last_backup {
                                span title=(t.format("%Y-%m-%d %H:%M UTC").to_string()) {
                                    (fmt_ago(t))
                                }
                            } @else {
                                span { "never" }
                            }
                        }
                    }
                }
            }
        }
    }))
}

async fn ui_events(State(app): State<AppState>) -> Result<Markup, StatusCode> {
    let rows: Vec<EventRow> = sqlx::query_as(
        r#"
        SELECT hostname, src, target, status, backup_end,
               total_bytes_processed, files_new, snapshot_id
        FROM restic_summary_msg
        ORDER BY backup_end DESC NULLS LAST
        LIMIT 100
        "#,
    )
    .fetch_all(&app.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(page("bk — events", html! {
        h2 { "Recent Events" }
        table {
            thead {
                tr {
                    th { "Host" } th { "Source" } th { "Target" }
                    th { "Status" } th { "End" } th { "Bytes" }
                    th { "New Files" } th { "Snapshot" }
                }
            }
            tbody {
                @for r in &rows {
                    @let end = r.backup_end.map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string()).unwrap_or_else(|| "—".into());
                    @let status = r.status.as_deref().unwrap_or("—");
                    @let cls = if status.eq_ignore_ascii_case("ok") { "ok" } else { "fail" };
                    @let bytes = r.total_bytes_processed.map(fmt_bytes).unwrap_or_else(|| "—".into());
                    tr {
                        td { (r.hostname) }
                        td { (r.src.as_deref().unwrap_or("—")) }
                        td { (r.target.as_deref().unwrap_or("—")) }
                        td { span class=(format!("chip {cls}")) { (status.to_uppercase()) } }
                        td { (end) }
                        td { (bytes) }
                        td { (r.files_new.map(|n| n.to_string()).unwrap_or_else(|| "—".into())) }
                        td style="font-size:.7rem;color:var(--muted)" { (r.snapshot_id.as_deref().unwrap_or("—")) }
                    }
                }
            }
        }
    }))
}

#[derive(sqlx::FromRow)]
struct HostStatsRow {
    total_backups: Option<i64>,
    total_bytes: Option<i64>,
    total_added: Option<i64>,
    avg_duration: Option<f64>,
    last_backup: Option<chrono::DateTime<chrono::Utc>>,
    last_status: Option<String>,
    added_7d: Option<i64>,
    added_30d: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct ForgetEventRow {
    target: Option<String>,
    removed: Option<i64>,
    kept: Option<i64>,
    dry_run: Option<bool>,
    status: Option<String>,
    timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(sqlx::FromRow)]
struct SnapshotRow {
    id: String,
    short_id: Option<String>,
    paths: Option<String>,
    tags: Option<String>,
    time: Option<chrono::DateTime<chrono::Utc>>,
    username: Option<String>,
}

#[derive(sqlx::FromRow)]
struct HostEventRow {
    src: Option<String>,
    target: Option<String>,
    status: Option<String>,
    backup_end: Option<chrono::DateTime<chrono::Utc>>,
    total_bytes_processed: Option<i64>,
    data_added_packed: Option<i64>,
    files_new: Option<i64>,
    total_duration: Option<f64>,
    snapshot_id: Option<String>,
}

async fn ui_host(
    State(app): State<AppState>,
    Path(hostname): Path<String>,
) -> Result<Markup, StatusCode> {
    let stats: HostStatsRow = sqlx::query_as(r#"
        SELECT
            COUNT(*)                                                        AS total_backups,
            SUM(total_bytes_processed)::BIGINT                              AS total_bytes,
            SUM(data_added_packed)::BIGINT                                  AS total_added,
            AVG(total_duration)                                             AS avg_duration,
            MAX(backup_end)                                                 AS last_backup,
            (SELECT status FROM restic_summary_msg
             WHERE hostname = $1 ORDER BY timestamp DESC LIMIT 1) AS last_status,
            (SELECT SUM(data_added_packed)::BIGINT FROM restic_summary_msg
             WHERE hostname = $1 AND backup_end > now() - interval '7 days') AS added_7d,
            (SELECT SUM(data_added_packed)::BIGINT FROM restic_summary_msg
             WHERE hostname = $1 AND backup_end > now() - interval '30 days') AS added_30d
        FROM restic_summary_msg
        WHERE hostname = $1
    "#)
    .bind(&hostname)
    .fetch_one(&app.db)
    .await
    .map_err(|e| { log::error!("host stats query failed: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;

    let events: Vec<HostEventRow> = sqlx::query_as(r#"
        SELECT src, target, status, backup_end,
               total_bytes_processed, data_added_packed, files_new, total_duration, snapshot_id
        FROM restic_summary_msg
        WHERE hostname = $1
        ORDER BY backup_end DESC NULLS LAST
        LIMIT 100
    "#)
    .bind(&hostname)
    .fetch_all(&app.db)
    .await
    .map_err(|e| { log::error!("host events query failed: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;

    let snapshots: Vec<SnapshotRow> = sqlx::query_as(r#"
        SELECT id, short_id, paths, tags, time, username
        FROM snapshots
        WHERE hostname = $1
        ORDER BY time DESC
        LIMIT 100
    "#)
    .bind(&hostname)
    .fetch_all(&app.db)
    .await
    .map_err(|e| { log::error!("snapshots query failed: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;

    let forget_events: Vec<ForgetEventRow> = sqlx::query_as(r#"
        SELECT target, removed, kept, dry_run, status, timestamp
        FROM forget_events
        WHERE hostname = $1
        ORDER BY timestamp DESC
        LIMIT 50
    "#)
    .bind(&hostname)
    .fetch_all(&app.db)
    .await
    .map_err(|e| { log::error!("forget events query failed: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;

    if stats.total_backups == Some(0) || stats.total_backups.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    let last_status = stats.last_status.as_deref().unwrap_or("unknown");
    let cls = host_class(stats.last_status.as_deref(), stats.last_backup);

    Ok(page(&format!("bk — {hostname}"), html! {
        a.back href="/" { "← all hosts" }
        div.host-title {
            div class=(format!("dot {cls}")) style="width:12px;height:12px" {}
            h1 { (hostname) }
            span class=(format!("badge {cls}")) { (last_status.to_uppercase()) }
        }

        div.stat-grid {
            div.stat {
                div.stat-label { "total backups" }
                div.stat-value { (stats.total_backups.unwrap_or(0)) }
            }
            div.stat {
                div.stat-label { "last backup" }
                div.stat-value style="font-size:.9rem" {
                    @if let Some(t) = stats.last_backup {
                        span title=(t.format("%Y-%m-%d %H:%M UTC").to_string()) { (fmt_ago(t)) }
                    } @else { "never" }
                }
            }
            div.stat {
                div.stat-label { "total processed" }
                div.stat-value { (fmt_bytes(stats.total_bytes.unwrap_or(0))) }
            }
            div.stat {
                div.stat-label { "total added" }
                div.stat-value { (fmt_bytes(stats.total_added.unwrap_or(0))) }
            }
            div.stat {
                div.stat-label { "avg duration" }
                div.stat-value {
                    (stats.avg_duration.map(|d| format!("{:.1}s", d)).unwrap_or_else(|| "—".into()))
                }
            }
            div.stat {
                div.stat-label { "added (7d)" }
                div.stat-value { (fmt_bytes(stats.added_7d.unwrap_or(0))) }
            }
            div.stat {
                div.stat-label { "added (30d)" }
                div.stat-value { (fmt_bytes(stats.added_30d.unwrap_or(0))) }
            }
            @if let (Some(w), Some(m)) = (stats.added_7d, stats.added_30d) {
                @let rate = if m > 0 { format!("{:.0}%", (w as f64 / m as f64) * 100.0) } else { "—".into() };
                div.stat {
                    div.stat-label { "7d / 30d ratio" }
                    div.stat-value style="font-size:1rem" { (rate) }
                }
            }
        }

        div.section {
            h2 { "Backup History" }
            table {
                thead {
                    tr {
                        th { "Time" } th { "Status" } th { "Source" } th { "Target" }
                        th { "Processed" } th { "Added" } th { "New Files" }
                        th { "Duration" } th { "Snapshot" }
                    }
                }
                tbody {
                    @for e in &events {
                        @let status = e.status.as_deref().unwrap_or("—");
                        @let ecls = if status.eq_ignore_ascii_case("ok") { "ok" } else { "fail" };
                        @let bytes = e.total_bytes_processed.map(fmt_bytes).unwrap_or_else(|| "—".into());
                        @let added = e.data_added_packed.map(fmt_bytes).unwrap_or_else(|| "—".into());
                        @let dur = e.total_duration.map(|d| format!("{:.1}s", d)).unwrap_or_else(|| "—".into());
                        tr {
                            td {
                                @if let Some(t) = e.backup_end {
                                    span title=(t.format("%Y-%m-%d %H:%M UTC").to_string()) { (fmt_ago(t)) }
                                } @else { "—" }
                            }
                            td { span class=(format!("chip {ecls}")) { (status.to_uppercase()) } }
                            td { (e.src.as_deref().unwrap_or("—")) }
                            td { (e.target.as_deref().unwrap_or("—")) }
                            td { (bytes) }
                            td { (added) }
                            td { (e.files_new.map(|n| n.to_string()).unwrap_or_else(|| "—".into())) }
                            td { (dur) }
                            td {
                                @if let Some(id) = &e.snapshot_id {
                                    code { (&id[..8.min(id.len())]) }
                                } @else { "—" }
                            }
                        }
                    }
                }
            }
        }

        @if !forget_events.is_empty() {
            div.section {
                h2 { "Prune History (" (forget_events.len()) ")" }
                table {
                    thead {
                        tr {
                            th { "Time" } th { "Target" } th { "Status" } th { "Removed" } th { "Kept" } th { "Mode" }
                        }
                    }
                    tbody {
                        @for f in &forget_events {
                            @let fstatus = f.status.as_deref().unwrap_or("ok");
                            @let fcls = if fstatus.eq_ignore_ascii_case("ok") { "ok" } else { "fail" };
                            tr {
                                td {
                                    @if let Some(t) = f.timestamp {
                                        span title=(t.format("%Y-%m-%d %H:%M UTC").to_string()) { (fmt_ago(t)) }
                                    } @else { "—" }
                                }
                                td style="color:var(--muted);font-size:.8rem" { (f.target.as_deref().unwrap_or("—")) }
                                td { span class=(format!("chip {fcls}")) { (fstatus.to_uppercase()) } }
                                td { span.chip.fail { (f.removed.unwrap_or(0)) " removed" } }
                                td { span.chip.ok  { (f.kept.unwrap_or(0)) " kept" } }
                                td style="color:var(--muted)" {
                                    @if f.dry_run == Some(true) { "dry run" } @else { "live" }
                                }
                            }
                        }
                    }
                }
            }
        }

        @if !snapshots.is_empty() {
            div.section {
                h2 { "Snapshots (" (snapshots.len()) ")" }
                table {
                    thead {
                        tr {
                            th { "ID" } th { "Time" } th { "Paths" } th { "Tags" } th { "User" }
                        }
                    }
                    tbody {
                        @for s in &snapshots {
                            tr {
                                td { code { (s.short_id.as_deref().unwrap_or(&s.id[..8])) } }
                                td {
                                    @if let Some(t) = s.time {
                                        span title=(t.format("%Y-%m-%d %H:%M UTC").to_string()) { (fmt_ago(t)) }
                                    } @else { "—" }
                                }
                                td style="font-size:.75rem;color:var(--muted)" {
                                    (s.paths.as_deref().unwrap_or("—"))
                                }
                                td style="font-size:.75rem;color:var(--muted)" {
                                    (s.tags.as_deref().filter(|t| !t.is_empty()).unwrap_or("—"))
                                }
                                td style="color:var(--muted)" {
                                    (s.username.as_deref().unwrap_or("—"))
                                }
                            }
                        }
                    }
                }
            }
        }
    }))
}

pub async fn persist_summary_msg(
    pool: &PgPool,
    msg: Option<ResticSummaryMsg>,
    src: &str,
    target: &str,
    status: &str,
    hostname: &str,
    sshid: &str,
) -> Result<(), StatusCode> {
    let m = msg.as_ref();
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
    .bind(m.and_then(|m| m.changed_snapshots))
    .bind(m.and_then(|m| m.files_new))
    .bind(m.and_then(|m| m.files_changed))
    .bind(m.and_then(|m| m.files_unmodified))
    .bind(m.and_then(|m| m.dirs_new))
    .bind(m.and_then(|m| m.dirs_changed))
    .bind(m.and_then(|m| m.dirs_unmodified))
    .bind(m.and_then(|m| m.data_blobs))
    .bind(m.and_then(|m| m.tree_blobs))
    .bind(m.and_then(|m| m.data_added))
    .bind(m.and_then(|m| m.data_added_packed))
    .bind(m.and_then(|m| m.total_files_processed))
    .bind(m.and_then(|m| m.total_bytes_processed))
    .bind(m.and_then(|m| m.total_duration))
    .bind(m.and_then(|m| m.backup_start))
    .bind(m.and_then(|m| m.backup_end))
            .bind(m.and_then(|m| m.snapshot_id.clone()))
            .bind(hostname)
            .bind(sshid)
            .bind(src)
    .bind(target)
    .bind(status)
    .execute(pool)
    .await
    .map_err(|e| { log::error!("DB insert failed: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(())
}
