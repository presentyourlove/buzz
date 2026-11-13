#!/bin/bash
# Buzz Agent Startup Script
# Usage: ./buzz-agent-start.sh [config_dir]
#   config_dir: Directory containing buzz-agent.env and health-guardian.env (default: /etc/buzz-agent)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
CONFIG_DIR="${1:-/etc/buzz-agent}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

# Check if running as root (for systemd) or as buzz user
if [[ $EUID -eq 0 ]]; then
    log_warn "Running as root. Consider running as dedicated 'buzz' user."
fi

# Validate config directory
if [[ ! -d "$CONFIG_DIR" ]]; then
    log_error "Config directory not found: $CONFIG_DIR"
    log_info "Create it and copy .env.example files:"
    log_info "  mkdir -p $CONFIG_DIR"
    log_info "  cp $PROJECT_ROOT/deploy/config/*.env.example $CONFIG_DIR/"
    log_info "  # Then edit the files with your configuration"
    exit 1
fi

# Load environment files
if [[ -f "$CONFIG_DIR/buzz-agent.env" ]]; then
    log_info "Loading $CONFIG_DIR/buzz-agent.env"
    set -a
    source "$CONFIG_DIR/buzz-agent.env"
    set +a
else
    log_warn "buzz-agent.env not found in $CONFIG_DIR"
fi

if [[ -f "$CONFIG_DIR/health-guardian.env" ]]; then
    log_info "Loading $CONFIG_DIR/health-guardian.env"
    set -a
    source "$CONFIG_DIR/health-guardian.env"
    set +a
else
    log_warn "health-guardian.env not found in $CONFIG_DIR (Health Guardian will be disabled)"
fi

# Set defaults
export BUZZ_RELAY_URL="${BUZZ_RELAY_URL:-http://localhost:3000}"
export BUZZ_WORK_LOGS_PATH="${BUZZ_WORK_LOGS_PATH:-$PROJECT_ROOT/.buzz/WORK_LOGS}"

# Create required directories
mkdir -p "$BUZZ_WORK_LOGS_PATH"
mkdir -p "$(dirname "${BUZZ_CIRCUIT_BREAKER_PATH:-$BUZZ_WORK_LOGS_PATH/circuit_breaker.json}")"
mkdir -p "$(dirname "${BUZZ_SYNC_PID_FILE:-$BUZZ_WORK_LOGS_PATH/sync_obsidian.pid}")"

# Validate required variables for Health Guardian
if [[ "${BUZZ_HEALTH_GUARDIAN_ENABLED:-0}" == "1" ]]; then
    if [[ -z "${BUZZ_HEALTH_GUARDIAN_CHANNEL:-}" ]]; then
        log_error "BUZZ_HEALTH_GUARDIAN_CHANNEL is required when Health Guardian is enabled"
        exit 1
    fi
    if [[ -z "${BUZZ_PRIVATE_KEY:-}" ]]; then
        log_error "BUZZ_PRIVATE_KEY is required for Health Guardian Nostr signing"
        exit 1
    fi
    log_success "Health Guardian configuration validated"
fi

# Validate agent identity for Mode B
if [[ -z "${BUZZ_PRIVATE_KEY:-}" ]]; then
    log_warn "BUZZ_PRIVATE_KEY not set - Mode B daemon will not start"
fi

# Check if buzz-agent binary exists
BUZZ_AGENT_BIN="${BUZZ_AGENT_BIN:-/usr/local/bin/buzz-agent}"
if [[ ! -x "$BUZZ_AGENT_BIN" ]]; then
    # Try to find in PATH or project target
    if command -v buzz-agent >/dev/null 2>&1; then
        BUZZ_AGENT_BIN="$(command -v buzz-agent)"
    elif [[ -x "$PROJECT_ROOT/target/release/buzz-agent" ]]; then
        BUZZ_AGENT_BIN="$PROJECT_ROOT/target/release/buzz-agent"
    elif [[ -x "$PROJECT_ROOT/target/debug/buzz-agent" ]]; then
        BUZZ_AGENT_BIN="$PROJECT_ROOT/target/debug/buzz-agent"
    else
        log_error "buzz-agent binary not found. Build with: cargo build --release -p buzz-agent"
        exit 1
    fi
fi

log_info "Using buzz-agent binary: $BUZZ_AGENT_BIN"

# Run pre-flight health check if Health Guardian enabled
if [[ "${BUZZ_HEALTH_GUARDIAN_ENABLED:-0}" == "1" ]]; then
    log_info "Running pre-flight health check..."
    if "$BUZZ_AGENT_BIN" health-check --json 2>/dev/null; then
        log_success "Pre-flight health check passed"
    else
        log_warn "Pre-flight health check failed (continuing anyway)"
    fi
fi

# Start the agent
log_info "Starting buzz-agent..."
exec "$BUZZ_AGENT_BIN"
