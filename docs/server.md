# bk server

The `bk serve` component provides centralized observability for bk deployments. It is a passive, append-only observer — it never participates in backup execution or data handling.

---

## Data Flow

```
conf.toml
    │
    ▼
bk run <config.toml>
    ├── rsync  (optional, pre-backup sync)
    └── restic backup ──────────────────────────► restic repository
                │                                  (local / sftp / s3)
                │ JSON summary output
                ▼
        ResticSummaryMsg { files_new, data_added,
                           snapshot_id, backup_start/end, … }
                │
                ▼
        BackupEmitSummary { target, src, status, summary }
                │
                │ serialize to JSON payload
                ▼
        ssh_sign(payload, /etc/ssh/ssh_host_ed25519_key)
                │
                ▼
        StateMessage {
            kind:        Backup
            hostname:    <machine hostname>
            fingerprint: SHA256 of host public key
            public_key:  full OpenSSH public key
            payload:     JSON string
            signature:   OpenSSH PEM signature
        }
                │
                │ POST /emit   (conf.home = "http://server:8080")
                ▼
bk serve <config.toml>   [axum :8080]
    │
    └── POST /emit
            ├── ssh_verify(public_key, payload, signature)
            │       └── UNAUTHORIZED (401) if invalid
            ├── parse BackupEmitSummary from payload
            └── INSERT INTO restic_summary_msg (postgres)
```

The backup run is fully independent of the server. If the server is unreachable, the backup still completes — only the emit is skipped.

---

## Signature Verification

Clients sign the JSON payload using their SSH host key:

```
ssh_key::SshSig::sign(&private_key, "bk", HashAlg::Sha256, payload)
```

The server verifies using a `ssh-keygen -Y verify` subprocess (the `SshSig::verify` method is `pub(crate)` in ssh-key 0.6.7 and cannot be called directly). The public key submitted with the request is used as the allowed signer — the server trusts any key that can produce a valid signature over the payload it received.

---

## Storage

Events are persisted to PostgreSQL in the `restic_summary_msg` table:

| Column                            | Type        | Description                         |
| --------------------------------- | ----------- | ----------------------------------- |
| `snapshot_id`                     | TEXT (PK)   | restic snapshot ID                  |
| `hostname`                        | TEXT        | emitting host                       |
| `sshid`                           | TEXT        | SHA256 fingerprint of host key      |
| `src`                             | TEXT        | backup source paths (`;`-separated) |
| `target`                          | TEXT        | restic repository                   |
| `status`                          | TEXT        | `OK` or error                       |
| `backup_start` / `backup_end`     | TIMESTAMPTZ | backup window                       |
| `timestamp`                       | TIMESTAMPTZ | server ingestion time               |
| `files_new`, `files_changed`, …   | BIGINT      | restic counters                     |
| `data_added`, `data_added_packed` | BIGINT      | bytes added                         |
| `total_bytes_processed`           | BIGINT      | total bytes scanned                 |
| `total_duration`                  | DOUBLE      | seconds                             |

All rows are immutable once inserted. There are no updates or deletes.

---

## Web UI

`bk serve` includes a built-in read-only web interface:

- `GET /` — fleet overview: one row per host, last backup time, last status
- `GET /events` — table of the 100 most recent backup events across all hosts

The UI is server-side rendered HTML, no JavaScript required.

---

## Configuration

Wire a client to a server by setting `home` in `conf.toml`:

```toml
home = "http://your-server:8080"
```

If `home` is not set, `bk run` completes normally and no event is emitted.

---

## Scope

The server **does not**:

- Schedule or trigger backups
- Access backup repositories or data
- Execute commands on hosts
- Store the backup data itself

The server **does**:

- Accept and verify signed backup event manifests
- Persist events in an append-only store
- Expose a read-only status UI
