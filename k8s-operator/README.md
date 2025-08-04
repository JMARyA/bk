# BK Operator

## Install
To install the bk operator, apply the `CRD`s first and then the operator manifests:

```sh
kubectl apply -f ./crds
kubectl apply -f ./manifests
```

This will install bk into the `bk-system` namespace.

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
