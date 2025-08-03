# BK Operator

## Backing up Pods
- Volumes
- Config?
- BackupCmd

Annotate Deployment/StatefulSet -> CronJob -> Backup

## Restore
Restore a POD to previous version

Restore -> Shutdown Deployment -> Start Job -> Restore -> Activate deployment

## Cluster Sync?
Keep restic snapshots in sync with the cluster for reference and DevUX with kubectl.
