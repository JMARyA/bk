# BK Operator

## Install
To install the bk operator, apply the `CRD`s first and then the operator manifests:

```sh
kubectl apply -f ./crds
kubectl apply -f ./manifests
```

This will install bk into the `bk-system` namespace.

## CRDs

### ResticRepository
This CRD represents a restic repository as a backup target.

### NodeBackup
This CRD represents a scheduled backup of a nodes filesystem.

## Backing up Pods
To backup the volumes of a pod, you have to annotate the respective `Deployment` / `Statefulset`.

The following annotations are required and will create a backup `CronJob` once added:
- `bk/repository`: The backup target. This is a reference to a `ResticRepository` within the same namespace.
- `bk/schedule`: The cron schedule to run on.

Additional parameters include:
- `bk/exclude`: Volumes to exclude, comma-seperated
- `bk/cephfs_snap`: Try to create a cephfs snapshot on these volumes, comma-seperated
