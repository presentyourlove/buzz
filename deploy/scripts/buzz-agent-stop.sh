#!/bin/bash
# Buzz Agent Graceful Shutdown Script
# Usage: ./buzz-agent-stop.sh [pid_file]
#   pid_file: Path to PID file (default: /var/run/buzz-agent/buzz-agent.pid)

set -euo pipefail

PID_FILE="${1:-/var/run/buzz-agent/buzz-agent.pid}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

# Check if PID file exists
if [[ ! -f "$PID_FILE" ]]; then
    log_warn "PID file not found: $PID_FILE"
    log_info "Attempting to find buzz-agent process..."
    
    PIDS=$(pgrep -f "buzz-agent" 2>/dev/null | grep -v "^$$\$" || true)
    if [[ -z "$PIDS" ]]; then
        log_info "No buzz-agent process found"
        exit 0
    fi
    
    log_info "Found buzz-agent PIDs: $PIDS"
    for PID in $PIDS; do
        log_info "Sending SIGTERM to $PID..."
        kill -TERM "$PID" 2>/dev/null || true
    done
    
    # Wait for graceful shutdown
    for i in {1..30}; do
        PIDS=$(pgrep -f "buzz-agent" 2>/dev/null | grep -v "^$$\$" || true)
        if [[ -z "$PIDS" ]]; then
            log_success "All buzz-agent processes stopped gracefully"
            exit 0
        fi
        sleep 1
    done
    
    # Force kill if still running
    log_warn "Processes still running after 30s, sending SIGKILL..."
    for PID in $PIDS; do
        kill -KILL "$PID" 2>/dev/null || true
    done
    log_success "All buzz-agent processes forcefully stopped"
    exit 0
fi

# Read PID from file
PID=$(cat "$PID_FILE" 2>/dev/null || echo "")
if [[ -z "$PID" ]] || ! [[ "$PID" =~ ^[0-9]+$ ]]; then
    log_error "Invalid PID in $PID_FILE: '$PID'"
    exit 1
fi

# Check if process exists
if ! kill -0 "$PID" 2>/dev/null; then
    log_warn "Process $PID not found (already stopped?)"
    rm -f "$PID_FILE"
    exit 0
fi

log_info "Stopping buzz-agent (PID: $PID)..."

# Send SIGTERM for graceful shutdown
kill -TERM "$PID" 2>/dev/null

# Wait for graceful shutdown (max 30 seconds)
for i in {1..30}; do
    if ! kill -0 "$PID" 2>/dev/null; then
        log_success "buzz-agent stopped gracefully"
        rm -f "$PID_FILE"
        exit 0
    fi
    sleep 1
done

# Force kill if still running
log_warn "Process $PID still running after 30s, sending SIGKILL..."
kill -KILL "$PID" 2>/dev/null || true
sleep 1

if kill -0 "$PID" 2>/dev/null; then
    log_error "Failed to stop process $PID"
    exit 1
fi

log_success "buzz-agent forcefully stopped"
rm -f "$PID_FILE"
