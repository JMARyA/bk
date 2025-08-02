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

## I want to backup

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
