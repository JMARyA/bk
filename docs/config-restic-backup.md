# Restic Backup Operation
This creates a snapshot with restic.

```toml
Restic Operation
[[restic]]
targets = ["my_restic_target"] 
src = [
   "my_path"
]

exclude = [
    "/exclude"
]

exclude_caches = true
exclude_if_present = [
    ".nobk"
]
reread = true
one_file_system = true
concurrency = 4
tags = [
    "tag1"
]

compression = "auto"
```

## Options
### `targets`
Specify [restic backup targets](./config-restic-target.md) for this backup by name.

### `src`
Specify [local paths](./config-path.md) to backup.

### `exclude`
exclude expressions

### `exclude_caches`
restic `--exclude-caches` option

### `exclude_if_present`
Exclude folders from backup if they contain these files.

### `reread`
Reread everything always

### `one_file_system`
Dont breach to other filesystems

### `concurrency`
Backup this many files concurrently

### `tags`
Additional snapshot tags

### `compression`
Restic compression
