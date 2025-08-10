# **ResticRepository**

Represents a remote backup repository location and its credentials.
A `ResticRepository` object tells the backup operator where to store backups and how to authenticate.

**API Definition**

| Field        | Type                            | Required | Description                                                                                                          |
| ------------ | ------------------------------- | -------- | -------------------------------------------------------------------------------------------------------------------- |
| `endpoint`   | `string`                        | ✅        | The repository endpoint. Must be either: <br>• S3 (`s3:s3.amazonaws.com/bucket`) <br>• SFTP (`sftp:user@host:/path`) |
| `s3`         | [`S3Config`](#s3config)         | ❌        | S3-specific credentials. Required if `endpoint` is an S3 bucket.                                                     |
| `ssh`        | [`SSHConfig`](#sshconfig)       | ❌        | SSH credentials for SFTP endpoints.                                                                                  |
| `passphrase` | [`SecretKeyRef`](#secretkeyref) | ✅        | Reference to a Kubernetes Secret containing the repository password.                                                 |

## **S3Config**

| Field        | Type                            | Required | Description                     |
| ------------ | ------------------------------- | -------- | ------------------------------- |
| `access_key` | [`SecretKeyRef`](#secretkeyref) | ✅        | Reference to the S3 access key. |
| `secret_key` | [`SecretKeyRef`](#secretkeyref) | ✅        | Reference to the S3 secret key. |

## **SSHConfig**

| Field        | Type                            | Required | Description                                               |
| ------------ | ------------------------------- | -------- | --------------------------------------------------------- |
| `secret_key` | [`SecretKeyRef`](#secretkeyref) | ✅        | Reference to the SSH private key used for authentication. |

## **SecretKeyRef**

| Field        | Type     | Required | Description                                       |
| ------------ | -------- | -------- | ------------------------------------------------- |
| `secretName` | `string` | ✅        | Name of the Kubernetes Secret containing the key. |
| `secretKey`  | `string` | ✅        | Key within the Secret that holds the value.       |

---

**Example: S3 Repository**

```yaml
apiVersion: bk.jmarya.me/v1
kind: ResticRepository
metadata:
  name: s3-repo
spec:
  endpoint: s3:s3.amazonaws.com/mybucket
  s3:
    access_key:
      secretName: s3-creds
      secretKey: accessKey
    secret_key:
      secretName: s3-creds
      secretKey: secretKey
  passphrase:
    secretName: restic-pass
    secretKey: password
```

**Example: SFTP Repository**

```yaml
apiVersion: bk.jmarya.me/v1
kind: ResticRepository
metadata:
  name: sftp-repo
spec:
  endpoint: sftp:backup@example.com:/var/backups
  ssh:
    secret_key:
      secretName: sftp-key
      secretKey: id_rsa
  passphrase:
    secretName: restic-pass
    secretKey: password
```
