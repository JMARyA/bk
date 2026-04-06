# S3 Input

S3 bucket input, mounted read-only via [geesefs](https://github.com/yandex-cloud/geesefs) (FUSE). The bucket appears as a local directory so restic can back it up without copying the data to disk first.

The mount path is stable across runs so restic deduplication works correctly:
- Running as root: `/var/cache/bk/s3/<key>`
- Running as a regular user: `$HOME/.cache/bk/s3/<key>`

```toml
[s3_input.my_bucket]
bucket = "my-bucket-name"
region = "us-east-1"
access_key = "AKIAIOSFODNN7EXAMPLE"
secret_key = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
```

Reference the key in a `[[restic]]` job the same way as a `[path]` input:

```toml
[[restic]]
targets = ["my_repo"]
src = ["my_bucket"]
```

## Options

### `bucket`
The S3 bucket name, optionally with a prefix (e.g. `"mybucket"` or `"mybucket/some/prefix"`).

### `endpoint`
Custom S3 endpoint URL for non-AWS providers (Minio, Backblaze, etc.).

```toml
endpoint = "https://s3.example.com"
```

### `region`
AWS region. Optional for non-AWS providers.

```toml
region = "us-east-1"
```

### `access_key` / `access_key_file`
Access key ID, either inline or read from a file.

```toml
access_key = "AKIAIOSFODNN7EXAMPLE"
# or
access_key_file = "/run/secrets/s3-access-key"
```

### `secret_key` / `secret_key_file`
Secret access key, either inline or read from a file.

```toml
secret_key = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
# or
secret_key_file = "/run/secrets/s3-secret-key"
```

## Requirements

`geesefs` must be installed and available in `PATH`. `bk run` will exit with an error immediately if it is missing.
