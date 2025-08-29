# Configuration
`bk` is configured trough a TOML config file.

The config file consists of global options and many module sections.

## Global Options
### `start_script`
Run a script before executing any backup tasks.

```toml
start_script = "pre.sh"
```

### `end_script`
Run a script after executing backup tasks.

```toml
end_script = "end.sh"
```

### `delay`
Wait a random delay before starting to backup. The value is the maximum wait time in seconds.

```toml
delay = 60 # randomized wait. 60 seconds max
```

## Sections
There are various module sections you can add to the config.

These can be divided into the following categories:

### Inputs
- [Path](./config-path.md)

### Targets
- [Restic](./config-restic-target.md)

### Operations
- [Rsync](./config-rsync.md)
- [Restic](./config-restic-backup.md)

### Notifications
- [Notifications](./notifications.md)
