# Design: Gitea + CI on jobrien-vm

**Date**: 2026-03-15
**Status**: Approved
**Scope**: Stand up Gitea (bare metal) with act_runner CI on jobrien-vm, push braid repo, add CI workflow

---

## 1. Target Machine

- **Host**: jobrien-vm (Tailscale IP: 100.105.75.7)
- **OS**: Ubuntu (kernel 6.8.0)
- **SSH**: user `dev`, password auth
- **Docker**: Not installed (not needed — bare metal install)

**Security notes**: Machine is Tailscale-only (no public internet exposure). HTTP (not HTTPS) is acceptable because Tailscale encrypts all traffic. The `dev` user uses password auth — acceptable for a private tailnet.

---

## 2. Gitea Server

### Installation

```bash
# Download Gitea binary (pin to latest stable)
wget -O /usr/local/bin/gitea https://dl.gitea.com/gitea/1.23/gitea-1.23-linux-amd64
chmod +x /usr/local/bin/gitea

# Create git user with login shell (needed for rustup and SSH)
useradd --system --shell /bin/bash --comment "Gitea" --create-home --home /home/git git

# Create directory structure
mkdir -p /var/lib/gitea/{data,log,custom/conf}
mkdir -p /home/git/gitea-repositories
chown -R git:git /var/lib/gitea
chown -R git:git /home/git
```

### Configuration (`/var/lib/gitea/custom/conf/app.ini`)

```ini
[server]
HTTP_PORT        = 3000
ROOT_URL         = http://100.105.75.7:3000/
SSH_DOMAIN       = 100.105.75.7
DOMAIN           = 100.105.75.7
DISABLE_SSH      = false
START_SSH_SERVER = true
SSH_PORT         = 2222

[database]
DB_TYPE  = sqlite3
PATH     = /var/lib/gitea/data/gitea.db

[repository]
ROOT = /home/git/gitea-repositories

[actions]
ENABLED = true
```

Key decisions:
- **SSH on port 2222** (built-in SSH server) to avoid conflicting with system SSH on 22
- **Actions enabled** for CI runner support
- **SQLite** — single-file DB, no external service needed
- **HTTP only** — Tailscale provides encryption

### Systemd Service (`/etc/systemd/system/gitea.service`)

```ini
[Unit]
Description=Gitea
After=network.target

[Service]
Type=simple
User=git
Group=git
WorkingDirectory=/var/lib/gitea/
ExecStart=/usr/local/bin/gitea web --config /var/lib/gitea/custom/conf/app.ini
Restart=always
Environment=USER=git HOME=/home/git GITEA_WORK_DIR=/var/lib/gitea

[Install]
WantedBy=multi-user.target
```

```bash
systemctl daemon-reload
systemctl enable --now gitea
```

### Admin User

Create `joe` admin user via CLI after first start:

```bash
GITEA_WORK_DIR=/var/lib/gitea sudo -u git /usr/local/bin/gitea admin user create \
  --admin --username joe --password <password> --email joe@localhost
```

---

## 3. act_runner (CI Runner)

### Installation

```bash
wget -O /usr/local/bin/act_runner https://dl.gitea.com/act_runner/0.2/act_runner-0.2-linux-amd64
chmod +x /usr/local/bin/act_runner
```

### Configuration and Registration

Generate default config, then register with Gitea:

```bash
# Generate default config
sudo -u git bash -c 'cd /var/lib/gitea && act_runner generate-config > act_runner.yaml'

# Get registration token from Gitea:
# Web UI → Site Administration → Runners → Create new Runner → copy token

# Register (non-interactive)
sudo -u git bash -c 'cd /var/lib/gitea && act_runner register \
  --no-interactive \
  --config /var/lib/gitea/act_runner.yaml \
  --instance http://localhost:3000 \
  --token <registration-token> \
  --name jobrien-runner \
  --labels ubuntu-latest:host'
```

The `ubuntu-latest:host` label maps the standard `runs-on: ubuntu-latest` to run directly on the host (no container).

**Security note**: The runner executes as the `git` user, which also owns Gitea data. This means CI jobs have access to the Gitea DB and other repos. Acceptable for single-user personal use.

### Systemd Service (`/etc/systemd/system/act_runner.service`)

```ini
[Unit]
Description=Gitea Act Runner
After=gitea.service

[Service]
Type=simple
User=git
Group=git
WorkingDirectory=/var/lib/gitea
ExecStart=/usr/local/bin/act_runner daemon --config /var/lib/gitea/act_runner.yaml
Restart=always
Environment=HOME=/home/git

[Install]
WantedBy=multi-user.target
```

```bash
systemctl daemon-reload
systemctl enable --now act_runner
```

---

## 4. Rust Toolchain on jobrien-vm

The CI runner executes `cargo` directly on the host, so Rust must be installed for the `git` user:

```bash
sudo -u git bash -c "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable"
sudo -u git bash -c 'source ~/.cargo/env && rustup component add clippy rustfmt'
```

The workflow will use whatever toolchain `rust-toolchain.toml` specifies. Rustup handles this automatically on first `cargo` invocation.

---

## 5. CI Workflow

File: `.gitea/workflows/ci.yml` in the braid repository.

```yaml
name: CI

on:
  push:
    branches: ['**']
  pull_request:

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Check formatting
        run: |
          source ~/.cargo/env
          cargo fmt --all -- --check

      - name: Clippy
        run: |
          source ~/.cargo/env
          cargo clippy --workspace -- -D warnings

      - name: Test
        run: |
          source ~/.cargo/env
          cargo test --workspace
```

Single job, three steps. Each step sources `~/.cargo/env` since the runner runs in host mode without a login shell. `runs-on: ubuntu-latest` maps to host execution via the runner label.

---

## 6. Braid Repo Push

From the local machine:

```bash
git remote add origin http://100.105.75.7:3000/joe/braid.git
git push -u origin main
```

Create the repo in Gitea first (via web UI or API), then push.

---

## 7. Build Order

1. Install Gitea binary, create `git` user, set up directories, set ownership
2. Write `app.ini` config
3. Create and start systemd service for Gitea (`systemctl enable --now`)
4. Create admin user, get runner registration token from web UI
5. Install Rust toolchain (stable + clippy + rustfmt) for `git` user
6. Install `act_runner`, generate config, register, create and start systemd service
7. Add `.gitea/workflows/ci.yml` to braid repo locally
8. Create repo in Gitea, add remote, push
9. Verify CI runs on push
