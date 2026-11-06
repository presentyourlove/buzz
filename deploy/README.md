# Buzz Agent Deployment Guide

This directory contains deployment artifacts for the Buzz Agent with System Health Guardian.

## Directory Structure

```
deploy/
├── systemd/
│   └── buzz-agent.service          # systemd service unit file
├── config/
│   ├── buzz-agent.env.example      # Core agent config (env format)
│   ├── health-guardian.env.example # Health Guardian config (env format)
│   └── buzz-agent.toml.example     # Unified config (TOML format)
├── scripts/
│   ├── buzz-agent-deploy.sh        # Full deployment script (run as root)
│   ├── buzz-agent-start.sh         # Startup script
│   ├── buzz-agent-stop.sh          # Graceful shutdown script
│   ├── buzz-agent-config-validate.sh # Config validation
│   └── buzz-agent-config-migrate.sh  # Env to TOML migration
└── README.md                       # This file
```

## Quick Start

### 1. Build the Binary

```bash
cd /path/to/buzz
cargo build --release -p buzz-agent
```

### 2. Deploy (as root)

```bash
sudo ./deploy/scripts/buzz-agent-deploy.sh
```

This will:
- Create `buzz` user and directories
- Install binary to `/usr/local/bin/buzz-agent`
- Install systemd service
- Install config templates to `/etc/buzz-agent/`
- Install management scripts to `/usr/local/bin/`

### 3. Configure

Edit the config files:

```bash
# Core agent config
sudo vim /etc/buzz-agent/buzz-agent.env

# Health Guardian config  
sudo vim /etc/buzz-agent/health-guardian.env
```

**Minimum required for Health Guardian:**
```bash
# In buzz-agent.env
BUZZ_AGENT_PROVIDER=anthropic
ANTHROPIC_API_KEY=sk-ant-...
BUZZ_PRIVATE_KEY=your-nostr-private-key
BUZZ_RELAY_URL=http://your-relay:3000

# In health-guardian.env
BUZZ_HEALTH_GUARDIAN_ENABLED=1
BUZZ_HEALTH_GUARDIAN_CHANNEL=your-channel-uuid
```

### 4. Validate Configuration

```bash
buzz-agent-config-validate
```

### 5. Start Service

```bash
systemctl start buzz-agent
systemctl enable buzz-agent  # Enable on boot
```

### 6. Check Status

```bash
systemctl status buzz-agent
journalctl -u buzz-agent -f
```

## Health Guardian Commands

### Manual Health Check
```bash
# Human-readable output
buzz-agent health-check

# JSON output
buzz-agent health-check --json
```

### Health Dashboard (Web UI)
```bash
# Start dashboard server
buzz-agent health-dashboard --port 8080 --host 0.0.0.0

# Then open http://your-server:8080/dashboard in browser
```

### API Endpoints
Once dashboard is running:
- `GET /health` - Simple status (healthy/degraded/critical)
- `GET /health/json` - JSON status summary
- `GET /health/markdown` - Full markdown report
- `GET /health/report` - Full JSON report
- `GET /health/circuit-breakers` - Circuit breaker status
- `GET /health/consecutive-failures` - Failure counter
- `GET /health/config` - Sanitized config
- `GET /dashboard` - Web dashboard UI

## Configuration Options

### Core Agent (buzz-agent.env)

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `BUZZ_AGENT_PROVIDER` | Yes | - | Provider: anthropic, openai, databricks, databricks_v2, openrouter |
| `ANTHROPIC_API_KEY` | For Anthropic | - | Anthropic API key |
| `OPENAI_COMPAT_API_KEY` | For OpenAI | - | OpenAI-compatible API key |
| `DATABRICKS_HOST` | For Databricks | - | Databricks workspace URL |
| `DATABRICKS_MODEL` | For Databricks | - | Model identifier |
| `OPENROUTER_API_KEY` | For OpenRouter | - | OpenRouter API key |
| `OPENROUTER_MODEL` | For OpenRouter | - | Model identifier |
| `BUZZ_AGENT_MODEL` | No | - | Universal model override |
| `BUZZ_PRIVATE_KEY` | For Mode B | - | Agent Nostr private key (hex/nsec) |
| `BUZZ_RELAY_URL` | For Mode B | http://localhost:3000 | Nostr relay URL |
| `BUZZ_AGENT_SYSTEM_PROMPT` | No | Built-in | System prompt |
| `BUZZ_AGENT_MAX_ROUNDS` | No | 0 | Max reasoning rounds |
| `BUZZ_AGENT_MAX_OUTPUT_TOKENS` | No | 65536 | Max output tokens |
| `BUZZ_AGENT_THINKING_EFFORT` | No | medium | Thinking effort level |
| `BUZZ_AGENT_THINKING_SUMMARY` | No | auto | Thinking summary mode |

### Health Guardian (health-guardian.env)

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `BUZZ_HEALTH_GUARDIAN_ENABLED` | Yes | 0 | Enable Health Guardian (1=true) |
| `BUZZ_HEALTH_GUARDIAN_CHANNEL` | If enabled | - | Nostr channel UUID for alerts |
| `BUZZ_HEALTH_GUARDIAN_SCHEDULE` | No | `*/15 * * * *` | Cron schedule for regular checks |
| `BUZZ_HEALTH_GUARDIAN_DAILY_DEEP` | No | `0 4 * * *` | Cron schedule for daily deep checks |
| `BUZZ_WORK_LOGS_PATH` | No | `./.buzz/WORK_LOGS` | Work logs directory |
| `BUZZ_CIRCUIT_BREAKER_PATH` | No | `./.buzz/WORK_LOGS/circuit_breaker.json` | Circuit breaker state file |
| `BUZZ_OPENROUTER_PORT` | No | 8080 | OpenRouter gateway port to monitor |
| `BUZZ_PROXY_PORT` | No | 3000 | Buzz proxy port to monitor |
| `BUZZ_HEALTH_MAX_CONSECUTIVE_FAILURES` | No | 3 | Alert threshold |
| `BUZZ_HEALTH_CB_AUTO_RESET` | No | true | Auto-reset circuit breakers |
| `BUZZ_HEALTH_RESTART_COOLDOWN` | No | 300 | Process restart cooldown (seconds) |

## TOML Configuration (Alternative)

For complex deployments, use TOML config:

```bash
# Migrate from env to TOML
buzz-agent-config-migrate /etc/buzz-agent /etc/buzz-agent/buzz-agent.toml

# Edit the TOML file
vim /etc/buzz-agent/buzz-agent.toml
```

The agent will automatically detect and use `buzz-agent.toml` if present.

## Systemd Service Management

```bash
# Start
systemctl start buzz-agent

# Stop
systemctl stop buzz-agent

# Restart
systemctl restart buzz-agent

# Status
systemctl status buzz-agent

# Logs
journalctl -u buzz-agent -f

# Enable on boot
systemctl enable buzz-agent

# Disable on boot
systemctl disable buzz-agent
```

## Health Guardian Features

### Phase A: On-Demand Health Check
- Runs all diagnostics (processes, ports, disk, memory)
- Analyzes circuit breakers
- Generates report (markdown/JSON)

### Phase B: Scheduled Guardian Daemon
- Runs on cron schedule (default: every 15 min)
- Daily deep check (default: 4 AM UTC)
- Automatic remediation:
  - Circuit breaker reset after recovery
  - Critical process restart
- Nostr alerts on consecutive failures
- Persistent failure counter across restarts

### Phase C: Deployment & Monitoring
- systemd service integration
- Health dashboard web UI
- Config validation & migration
- Structured logging

## Monitoring Integration

### Prometheus Metrics (Future)
The dashboard endpoints can be scraped by Prometheus:
- `/health/json` for status
- Custom metrics exporter (planned)

### Alerting
Health Guardian sends Nostr Kind 1 events to configured channel with tags:
- `h` = channel UUID
- `t` = "health-alert"

## Troubleshooting

### Service won't start
```bash
# Check config
buzz-agent-config-validate

# Check binary
/usr/local/bin/buzz-agent health-check --json

# Check logs
journalctl -u buzz-agent -n 100
```

### Health Guardian not running
```bash
# Verify env vars
cat /etc/buzz-agent/health-guardian.env

# Check if enabled
grep BUZZ_HEALTH_GUARDIAN_ENABLED /etc/buzz-agent/health-guardian.env

# Manual test
BUZZ_HEALTH_GUARDIAN_ENABLED=1 BUZZ_HEALTH_GUARDIAN_CHANNEL=xxx buzz-agent health-check
```

### Dashboard not accessible
```bash
# Check if running
buzz-agent health-dashboard --port 8080

# Check firewall
ss -tlnp | grep 8080
```

## File Locations After Deployment

| Component | Path |
|-----------|------|
| Binary | `/usr/local/bin/buzz-agent` |
| Systemd service | `/etc/systemd/system/buzz-agent.service` |
| Config dir | `/etc/buzz-agent/` |
| Work logs | `/opt/buzz-agent/.buzz/WORK_LOGS/` |
| Circuit breaker | `/opt/buzz-agent/.buzz/WORK_LOGS/circuit_breaker.json` |
| Logs | `/var/log/buzz-agent/` |
| PID file | `/var/run/buzz-agent/buzz-agent.pid` |

## Upgrading

```bash
# 1. Build new binary
cargo build --release -p buzz-agent

# 2. Stop service
systemctl stop buzz-agent

# 3. Replace binary
cp target/release/buzz-agent /usr/local/bin/buzz-agent

# 4. Start service
systemctl start buzz-agent

# 5. Verify
buzz-agent health-check --json
```
