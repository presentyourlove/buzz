#!/bin/bash
# Daily Evolution Cron Wrapper
# Runs the daily evolution script and posts summary to channel via buzz CLI
# Usage: run_daily_evolution.sh [--dry-run] [--no-post]

set -euo pipefail

# Configuration - override via environment variables
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="${WORKSPACE_ROOT:-$(cd "$SCRIPT_DIR/../.." && pwd)}"
CHANNEL_ID="${BUZZ_CHANNEL_ID:-5ca019f6-1a27-592a-b015-4baf0eb33572}"
RELAY_URL="${BUZZ_RELAY_URL:-}"
PRIVATE_KEY="${BUZZ_PRIVATE_KEY:-}"
BUZZ_CLI="${BUZZ_CLI:-buzz}"

# Parse arguments
DRY_RUN=false
NO_POST=false
for arg in "$@"; do
    case $arg in
        --dry-run) DRY_RUN=true ;;
        --no-post) NO_POST=true ;;
        *) echo "Unknown argument: $arg" >&2; exit 1 ;;
    esac
done

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() {
    echo -e "${GREEN}[$(date -u '+%Y-%m-%d %H:%M:%S UTC')]${NC} $*"
}

error() {
    echo -e "${RED}[$(date -u '+%Y-%m-%d %H:%M:%S UTC')] ERROR:${NC} $*" >&2
}

warn() {
    echo -e "${YELLOW}[$(date -u '+%Y-%m-%d %H:%M:%S UTC')] WARN:${NC} $*"
}

info() {
    echo -e "${BLUE}[$(date -u '+%Y-%m-%d %H:%M:%S UTC')] INFO:${NC} $*"
}

# Validate required environment
if [[ -z "$RELAY_URL" ]]; then
    error "BUZZ_RELAY_URL not set"
    exit 1
fi

if [[ -z "$PRIVATE_KEY" ]]; then
    error "BUZZ_PRIVATE_KEY not set"
    exit 1
fi

# Check if buzz CLI exists
if ! command -v "$BUZZ_CLI" &> /dev/null; then
    error "buzz CLI not found in PATH (looked for: $BUZZ_CLI)"
    exit 1
fi

log "Starting daily evolution for channel: $CHANNEL_ID"
log "Workspace: $WORKSPACE_ROOT"
[[ "$DRY_RUN" == "true" ]] && warn "DRY RUN MODE - no proposal saved, no channel post"
[[ "$NO_POST" == "true" ]] && warn "NO POST MODE - proposal saved but no channel post"

# Run the daily evolution script
cd "$WORKSPACE_ROOT"

# Build python command
PYTHON_CMD=(
    python3 "$SCRIPT_DIR/daily_evolution.py"
    --channel "$CHANNEL_ID"
    --relay-url "$RELAY_URL"
    --private-key "$PRIVATE_KEY"
    --buzz-cli "$BUZZ_CLI"
    --workspace "$WORKSPACE_ROOT"
)

[[ "$DRY_RUN" == "true" ]] && PYTHON_CMD+=(--dry-run)
[[ "$NO_POST" == "true" ]] && PYTHON_CMD+=(--no-post)

# Run and capture output
info "Executing: ${PYTHON_CMD[*]}"
OUTPUT=$(PYTHONPATH="$WORKSPACE_ROOT" "${PYTHON_CMD[@]}" 2>&1)
EXIT_CODE=$?

# Print output for logging
echo "$OUTPUT"

if [[ $EXIT_CODE -ne 0 ]]; then
    error "Daily evolution script failed with exit code $EXIT_CODE"
    exit $EXIT_CODE
fi

# Try to parse JSON output for success confirmation
if [[ "$DRY_RUN" == "false" ]]; then
    # Extract last line which should be JSON
    JSON_LINE=$(echo "$OUTPUT" | tail -1)
    if echo "$JSON_LINE" | grep -q '"success":true'; then
        EVENT_ID=$(echo "$JSON_LINE" | grep -o '"posted_event_id":"[^"]*"' | cut -d'"' -f4)
        if [[ -n "$EVENT_ID" && "$EVENT_ID" != "null" ]]; then
            log "Summary posted to channel (event: $EVENT_ID)"
        else
            warn "Script succeeded but no event ID in response"
        fi
    else
        warn "Script exited 0 but JSON output not detected"
    fi
fi

log "Daily evolution completed successfully"
log "Done"
