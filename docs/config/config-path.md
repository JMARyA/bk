# Path Input
Local Path Input which you can later reference by name.

```toml
[path.my_path]
path = "/my_path"
ensure_exists = "/dir"
cephfs_snap = true
same_path = true
```

## Options
### `path`
The local path

### `ensure_exists`
Errors if the directory is empty.

### `cephfs_snap`
Makes a cephfs snapshot and transforms the path.

### `same_path`
Bind mounts the path to a consistent path and transforms the path.
