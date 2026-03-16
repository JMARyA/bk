# bk

Declarative backup runner with centralized observability. Define your sources, targets, and retention rules in a TOML config — `bk` runs restic (and optionally rsync), then reports the result to a central server.

## Install

Via Nix flake:

```nix
inputs.bk.url = "git+https://git.hydrar.de/jmarya/bk";
```

Or build from source:

```sh
cargo build --release
```

---

## Quick start — local backup

**1. Create a config file**

```toml
# bk.toml

[path.home]
path = "/home/alice"

[restic_target.local]
repo = "/backup/home.restic"
passphrase = "changeme"

[[restic]]
src     = ["home"]
targets = ["local"]
exclude         = [".cache", ".local/share/Trash"]
exclude_caches  = true
one_file_system = true
```

**2. Initialize the repository**

```sh
bk init bk.toml
```

**3. Run a backup**

```sh
bk run bk.toml
```

**4. List snapshots**

```sh
bk list bk.toml
```

**5. Restore a snapshot**

```sh
bk restore bk.toml local <snapshot-id> /tmp/restore
```

---

## Backup targets

### Local filesystem

```toml
[restic_target.local]
repo       = "/backup/home.restic"
passphrase = "changeme"
```

### Remote over SSH (sftp)

```toml
[restic_target.offsite]
repo         = "sftp:myhost:/backup/home.restic"
passphrase   = "changeme"
ssh.identity = "/home/alice/.ssh/id_ed25519"
ssh.port     = 22
```

### S3-compatible object storage

```toml
[restic_target.s3]
repo                  = "s3:s3.example.com/my-bucket"
passphrase_file       = "/run/secrets/restic-pass"
s3.access_key_file    = "/run/secrets/s3-access-key"
s3.secret_key_file    = "/run/secrets/s3-secret-key"
```

---

## Scheduling

### systemd (user session)

```ini
# ~/.config/systemd/user/bk.service
[Unit]
Description=bk backup

[Service]
Type=simple
ExecStart=/usr/bin/bk run /home/alice/.config/bk.toml
StandardOutput=journal
StandardError=journal
```

```ini
# ~/.config/systemd/user/bk.timer
[Unit]
Description=daily bk backup

[Timer]
OnCalendar=daily
Persistent=true

[Install]
WantedBy=timers.target
```

```sh
systemctl --user enable --now bk.timer
```

### NixOS module

```nix
{
  services.bk = {
    enable = true;
    state  = [ "/var/lib/myapp" "/home" ];
    repo   = "sftp:myhost:/backup/myhost.restic";
    repoOptions.passphrase_file = "/run/secrets/restic-pass";
  };
}
```

This generates `/etc/bk.toml` and a systemd timer that runs daily.

---

## Centralized observability server

`bk serve` collects signed backup events from all your hosts into a single Postgres database and exposes a web UI.

### Server setup

**1. Set `DATABASE_URL` and start the server**

```sh
export DATABASE_URL=postgres://bk:password@localhost/bk
bk serve server.toml
```

The server listens on `:8080`, runs migrations automatically, and serves the UI at `http://localhost:8080`.

**2. `server.toml`** (minimal — no backup sources needed on the server itself)

```toml
# server.toml
# no fields required for a serve-only node
```

### Point clients at the server

Add `home` to each client's config:

```toml
# bk.toml (on each client host)
home = "http://bk-server:8080"

[path.data]
path = "/var/lib/myapp"

[restic_target.offsite]
repo       = "sftp:backup-host:/repos/myapp.restic"
passphrase = "changeme"

[[restic]]
src     = ["data"]
targets = ["offsite"]
```

After `bk run`, the client signs the backup summary with its SSH host key (`/etc/ssh/ssh_host_ed25519_key`) and POSTs it to the server. The server verifies the signature before persisting.

### Web UI

| Route         | Description                                                 |
| ------------- | ----------------------------------------------------------- |
| `GET /`       | Fleet overview — one row per host, last backup time, status |
| `GET /events` | Last 100 backup events across all hosts                     |

### How it works

```
bk run  →  restic backup  →  sign summary with SSH host key
                                        │
                                        ▼
                              POST /emit  (StateMessage)
                                        │
                                        ▼
                              bk serve  →  verify signature  →  postgres
                                                                    │
                                                                    ▼
                                                              web UI / queries
```

No backup data or credentials ever touch the server — only the restic summary metadata (file counts, bytes, snapshot ID, timing).

---

## All commands

```
bk init <config>                                   initialize restic repositories
bk run <config>                                    run all backup operations
bk list <config>                                   list snapshots
bk restore <config> <target> <snapshot> <dest>     restore a snapshot
bk show <config>                                   show parsed config
bk config_schema                                   print JSON schema for config
bk serve <config>                                  run observability server
```

---

## Further reading

- [Configuration reference](./docs/config/config.md)
- [Server architecture](./docs/server.md)
