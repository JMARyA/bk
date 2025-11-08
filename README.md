# 🍔 BK
`bk` is a backup utility.

## Configuration
You run `bk` against a configuration file in TOML format.

See [Configuration](./docs/config.md) for more details.

## I want to backup

### Requirements
You need at least one backup location.

This can be either:

#### Local Filesystem Target
To use a local filesystem:

```toml
[restic_target.my_restic_target]
repo = "/backup/repo.restic"
passphrase = "password"
```

#### SSH Remote Target
Add this to your `bk.toml` configuration:

```toml
[restic_target.offsite]
repo = "sftp:myhost:/backup/repo.restic"
passphrase = "password"
```

For simple auth, add this to `.ssh/config`:

```
Host myhost
    Hostname <hostname>
    User <user>
    IdentityFile <ssh_key>
```

#### S3 Remote Target
#todo

### Local Machine
#### Nixos
To backup a Nixos machine, add `inputs.bk.url = "git+https://git.hydrar.de/jmarya/bk";` to your flake inputs.

To use `bk` include the Nixos Module at `inputs.bk.nixosModules.bk`.

Sample configuration:

```nix
{
  services.bk = {
    enable = true;

    # The three options below is all thats needed for a basic backup

    # State folders to backup
    state = [
      "/somepath"
    ]:

    # Backup repository
    repo = "sftp:myhost:/backup/repo.restic"

    # Options for the repository
    repoOptions = {
      passphrase_file = "/secret";
    }

    # Global configuration settings
    globalSettings = { };

    # Manual Configuration Blocks which get merged. Can contain raw config options or made with helper functions from inputs.bk.lib.
    settings = [
      # mkBk automatically sets up paths, targets and a basic restic operation
      (inputs.lib.bk.mkBk {
              paths = [ "/otherpath" ];
              repo = "/otherrepo";
              # Extra Options for the restic operation
              extraOptions = {};
              # Extra options for the paths
              extraPathOptions = { };
              # extra options for the target
              extraTargetOptions = { };
      })
    ];

  };
}
```

This automatically builds a `/etc/bk.toml` config file and sets up systemd services for backup once a day.

#### Home Folder

Create systemd units:

```ini
# /home/<user>/.config/systemd/user/backup.service
[Unit]
Description=Home Folder Backup

[Service]
Type=simple
ExecStart=/usr/bin/bk /home/<user/.config/bk.toml
StandardOutput=journal
StandardError=journal
```

```ini
# /home/<user>/.config/systemd/user/backup.timer
[Unit]
Description=Scheduled backup

[Timer]
OnCalendar=daily
Persistent=true

[Install]
WantedBy=timers.target
```

Then a config file:

```toml
# /home/<user>/.config/bk.toml
[path.homedir]
path = "/home/<user>"

[restic_target.offsite]
repo = "<repo>"
passphrase = "<passphrase>"

[[restic]]
src = ["homedir"]
targets = ["offsite"] 

exclude = [
    ".cache", # Caches
    ".local/share/Trash", # System Trash
    ".local/share/containers" # Containers
]
exclude_caches = true
one_file_system = true

concurrency = 4
```

Run first backup and tests with:
```shell
bk ~/.config/bk.toml
```

And enable backups with:

```shell
systemctl --user enable --now backup.timer
```
