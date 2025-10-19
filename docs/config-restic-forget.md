# Restic Forget Operation
This forgets (prunes) snapshots of a restic target.

```toml
Restic Operation
[[restic_forget]]
targets = ["my_restic_target"] 

keep_last = 40
```
