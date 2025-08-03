# Restic Target
A restic repository you can reference by name.

```toml
[restic_target.my_restic_target]
repo = "/backup/repo.restic"
passphrase = "password"
```

## Options
### `repo`
The URL to the repository. Can also be `sftp`.

### `passphrase`
The passphrase for the repository.
