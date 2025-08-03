# Restic Target
A restic repository you can reference by name.

```toml
[restic_target.my_restic_target]
repo = "/backup/repo.restic"
passphrase = "password"

s3.access_key = <key>
s3.secret_key = <key>

ssh.port = 22
ssh.identity = /root/.ssh/id_rsa
```

## Options
### `repo`
The URL to the repository. Can also be `sftp`.

### `passphrase`
The passphrase for the repository.

### `s3.access_key`
optional auth key for s3

### `s3.secret_key`
optional auth key for s3

### `ssh.port`
The port for SSH

### `ssh.identity`
Path to the IdentityFile
