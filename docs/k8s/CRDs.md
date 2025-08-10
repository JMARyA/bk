# **NodeBackup**

Represents a scheduled backup of specified paths on a specific Kubernetes node to a configured repository.

**API Definition**

| Field         | Type       | Required | Description                                                      |
| ------------- | ---------- | -------- | ---------------------------------------------------------------- |
| `repository`  | `string`   | ✅        | Name of the `ResticRepository` to use.                           |
| `paths`       | `[]string` | ✅        | List of absolute filesystem paths to back up.                    |
| `exclude`     | `[]string` | ❌        | List of patterns or paths to exclude from the backup.            |
| `schedule`    | `string`   | ✅        | Cron expression defining when backups run.                       |
| `node`        | `string`   | ✅        | The Kubernetes node where the backup job should execute.         |
| `quiet`       | `bool`     | ❌        | If true, suppresses most output during backup.                   |
| `cephfs_snap` | `bool`     | ❌        | If true, create a CephFS snapshot before running the backup.     |
| `same_path`   | `bool`     | ❌        | If true, keeps changing paths stored under the same backup path. |

---

**Example: Scheduled Node Backup**

```yaml
apiVersion: bk.jmarya.me/v1
kind: NodeBackup
metadata:
  name: etc-backup
spec:
  repository: s3-repo
  paths:
    - /etc
    - /var/lib/config
  exclude:
    - /etc/ssl
    - /var/lib/config/tmp
  schedule: "0 3 * * *" # Every day at 3 AM
  node: worker-node-1
  quiet: true
  cephfs_snap: false
```
