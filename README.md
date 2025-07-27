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
