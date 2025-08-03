# 🍔 BK
`bk` is a backup utility.

# v2 Mission
k8s operator with CDRs:
- `Backup`: Specify what (pods) to backup to (targets)
- `Schedule`: Define a cron schedule for (backup) to run. Will create managed cronjob.
- `Target`: Backup target

Support for anotations like:
- `bk.backupcmd`: Command to run and backup its `stdout`

CDRs are namespace bounded.

## Modularize
Input: `Path`, `Raw`, `Command`
Target: `Filesystem`, `Restic`, `SSH`

## Notification
Notifications when backups fail, or not.
Via mail, webhook, etc

```toml
[ntfy.my_ntfy.ntfy]
host = <host>
topic = <topic>
auth.user = <user>
auth.pass = <pass>
```

And reference in a backup:
```toml
[[restic]]
...
ntfy = ["my_ntfy"]
```

## I want to backup

### Requirements
You need at least one backup location.

This can be either:
- [Local Filesystem](#local-filesystem-target)
- [SSH Remote](#ssh-remote-target)
- [S3 Remote](#s3-remote-target)

#### Local Filesystem Target
To use a local filesystem:

```toml
[restic_target.my_restic_target]
repo = "/backup/repo.restic"
passphrase = "password"
```

#### SSH Remote Target
Add this to your `bk.toml` configuration:

```toml
[restic_target.offsite]
repo = "sftp:myhost:/backup/repo.restic"
passphrase = "password"
```

For simple auth, add this to `.ssh/config`:

```
Host myhost
    Hostname <hostname>
    User <user>
    IdentityFile <ssh_key>
```

#### S3 Remote Target
#todo

### Kubernetes
#todo: docs

### Local Machine
#todo: docs

#### Home Folder

Create systemd units:

```ini
# /home/<user>/.config/systemd/user/backup.service
[Unit]
Description=Home Folder Backup

[Service]
Type=simple
ExecStart=/usr/bin/bk /home/<user/.config/bk.toml
StandardOutput=journal
StandardError=journal
```

```ini
# /home/<user>/.config/systemd/user/backup.timer
[Unit]
Description=Scheduled backup

[Timer]
OnCalendar=daily
Persistent=true

[Install]
WantedBy=timers.target
```

Then a config file:

```toml
# /home/<user>/.config/bk.toml
[path.homedir]
path = "/home/<user>"

[restic_target.offsite]
repo = "<repo>"
passphrase = "<passphrase>"

[[restic]]
src = ["homedir"]
targets = ["offsite"] 

exclude = [
    ".cache", # Caches
    ".local/share/Trash", # System Trash
    ".local/share/containers" # Containers
]
exclude_caches = true
one_file_system = true

concurrency = 4
```

Run first backup and tests with:
```shell
bk ~/.config/bk.toml
```

And enable backups with:

```shell
systemctl --user enable --now backup.timer
```
