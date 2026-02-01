## Notifications
You can receive notifications for your backup jobs.

Set up notifier:

```toml
[ntfy.my_ntfy.ntfy]
host = <host>
topic = <topic>
auth.user = <user>
auth.pass = <pass>
# auth.pass_file = <file>
```

And reference in a backup:
```toml
[[restic]]
...
ntfy = ["my_ntfy"]
```

## Notifier
Notifiers receive the notifications from the backup jobs.
You can then specify different channels they get send to.

### ntfy.sh

```toml
[ntfy.<name>.ntfy]
host = <host>
topic = <topic>
auth.user = <user>
auth.pass = <pass>
```
