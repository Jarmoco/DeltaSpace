# DeltaSpace

**DeltaSpace** is a _lightweight, zero crate dependency_ **filesystem snapshot and diff explorer** tool for Linux and MacOS.

![DeltaSpace Screenshot](.github/screenshot.png)

## Features

- Scan filesystem and save a snapshot
- Compare snapshots
- Prune snapshots
- TUI for interactive usage
- CLI arguments for programmatic use

## Installation 
You can manually install from [releases](https://github.com/Jarmoco/DeltaSpace/releases) or use [Homebrew](https://brew.sh/):

```bash
brew tap Jarmoco/deltaspace
brew install deltaspace
```

one-liner:
```bash
brew install jarmoco/deltaspace/deltaspace
```

## Usage

### Interactive mode

```bash
./deltaspace
```

### CLI mode

```bash
./deltaspace <command> [args]
```

for help, run:

```bash
./deltaspace -h
```

## Automatic Scans

You can schedule automatic filesystem scans using your OS's task scheduler.

### Linux (systemd)

Create a user-level service at `~/.config/systemd/user/deltaspace-scan.service`:

```ini
[Unit]
Description=Run deltaspace scan

[Service]
Type=oneshot
ExecStart=/usr/bin/deltaspace scan
```

Create a timer at `~/.config/systemd/user/deltaspace-scan.timer`:

```ini
[Unit]
Description=Run deltaspace scan every 2 hours

[Timer]
OnCalendar=*-*-* 00/2:00:00
Persistent=true
RandomizedDelaySec=900

[Install]
WantedBy=timers.target
```

Enable lingering (so the timer runs without an active login session) and start the timer:

```bash
sudo loginctl enable-linger $USER
systemctl --user daemon-reload
systemctl --user enable --now deltaspace-scan.timer
```

## Performance

Tested on my system, it created a snapshot of ~127k directories in 6.5s.
Compilation time is around <2s due to the absence of dependencies.

## Building

To build the code you need to have `cargo` and `nfpm` installed.
To cross compile from linux to macos, you also need `zig` and `cargo-zigbuild` installed and the `aarch64-apple-darwin` target added to rustup.

Then run:

```bash
./build.sh
```

This will create the packages in the `dist/` directory.
