# Rsync
Simple rsync operation.

```toml
# Rsync Operation
[[rsync]]
# Directories SHOULD have trailing `/`
src = "/home/me/"
dest = "/backup/home/me/"

# Excludes
exclude = [".cache", ".local"]

# Delete entries not present in `src` from `destination`
delete = true

# Ensure this directory exists and it not empty before running rsync
ensure_exists = "/home"

# Make a CephFS snapshot before rsync
cephfs_snap = true
```
