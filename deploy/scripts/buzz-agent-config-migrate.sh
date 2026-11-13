#!/bin/bash
# Buzz Agent Configuration Migration Script
# Migrates from environment variable config to TOML config

set -euo pipefail

CONFIG_DIR="${1:-/etc/buzz-agent}"
OUTPUT_FILE="${2:-$CONFIG_DIR/buzz-agent.toml}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

# Load environment files
if [[ -f "$CONFIG_DIR/buzz-agent.env" ]]; then
    set -a
    source "$CONFIG_DIR/buzz-agent.env"
    set +a
fi

if [[ -f "$CONFIG_DIR/health-guardian.env" ]]; then
    set -a
    source "$CONFIG_DIR/health-guardian.env"
    set +a
fi

escape_toml() {
    local input="$1"
    # Escape quotes and backslashes safely
    printf '%s' "$input" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

generate_toml() {
    cat <<TOMLEOF
# Buzz Agent Configuration (TOML format)
# Generated from environment variables on $(date)
# 
# This file was auto-generated. Review and edit as needed.

[agent]
provider = "$(escape_toml "${BUZZ_AGENT_PROVIDER:-}")"
model = "$(escape_toml "${BUZZ_AGENT_MODEL:-}")"
private_key = "$(escape_toml "${BUZZ_PRIVATE_KEY:-}")"
relay_url = "$(escape_toml "${BUZZ_RELAY_URL:-http://localhost:3000}")"
system_prompt = """$(escape_toml "${BUZZ_AGENT_SYSTEM_PROMPT:-You are buzz-agent. Use the provided tools to act. Tool calls are your only output.}")"""
max_sessions = ${BUZZ_AGENT_MAX_SESSIONS:-100}
max_line_bytes = ${BUZZ_AGENT_MAX_LINE_BYTES:-4194304}
llm_timeout_secs = ${BUZZ_AGENT_LLM_TIMEOUT_SECS:-240}
tool_timeout_secs = ${BUZZ_AGENT_TOOL_TIMEOUT_SECS:-1260}
mcp_init_timeout_secs = ${BUZZ_AGENT_MCP_INIT_TIMEOUT_SECS:-30}
mcp_max_restart_attempts = ${BUZZ_AGENT_MCP_RESTART_MAX_ATTEMPTS:-3}
mcp_restart_base_ms = ${BUZZ_AGENT_MCP_RESTART_BASE_MS:-500}
mcp_restart_max_ms = ${BUZZ_AGENT_MCP_RESTART_MAX_MS:-30000}
thinking_effort = "$(escape_toml "${BUZZ_AGENT_THINKING_EFFORT:-medium}")"
thinking_summary = "$(escape_toml "${BUZZ_AGENT_THINKING_SUMMARY:-auto}")"
hints_enabled = ${BUZZ_AGENT_HINTS_ENABLED:-true}
history_cache_ttl_secs = ${BUZZ_AGENT_HISTORY_CACHE_TTL_SECS:-3600}
history_cache_max_entries = ${BUZZ_AGENT_HISTORY_CACHE_MAX:-10000}
databricks_host = "$(escape_toml "${DATABRICKS_HOST:-}")"
databricks_token = "$(escape_toml "${DATABRICKS_TOKEN:-}")"
databricks_model = "$(escape_toml "${DATABRICKS_MODEL:-}")"
databricks_model_filter = "$(escape_toml "${DATABRICKS_MODEL_FILTER:-}")"
openai_compat_base_url = "$(escape_toml "${OPENAI_COMPAT_BASE_URL:-https://api.openai.com/v1}")"
openai_compat_api = "$(escape_toml "${OPENAI_COMPAT_API:-auto}")"
openrouter_base_url = "$(escape_toml "${OPENROUTER_BASE_URL:-https://openrouter.ai/api/v1}")"

[health_guardian]
enabled = ${BUZZ_HEALTH_GUARDIAN_ENABLED:-false}
channel_id = "$(escape_toml "${BUZZ_HEALTH_GUARDIAN_CHANNEL:-}")"
schedule = "$(escape_toml "${BUZZ_HEALTH_GUARDIAN_SCHEDULE:-*/15 * * * *}")"
daily_deep_schedule = "$(escape_toml "${BUZZ_HEALTH_GUARDIAN_DAILY_DEEP:-0 4 * * *}")"
work_logs_path = "$(escape_toml "${BUZZ_WORK_LOGS_PATH:-./.buzz/WORK_LOGS}")"
circuit_breaker_path = "$(escape_toml "${BUZZ_CIRCUIT_BREAKER_PATH:-./.buzz/WORK_LOGS/circuit_breaker.json}")"
openrouter_port = ${BUZZ_OPENROUTER_PORT:-8080}
sync_pid_file = "$(escape_toml "${BUZZ_SYNC_PID_FILE:-./.buzz/sync_obsidian.pid}")"
proxy_port = ${BUZZ_PROXY_PORT:-3000}

[health_guardian.alert_threshold]
max_consecutive_failures = ${BUZZ_HEALTH_MAX_CONSECUTIVE_FAILURES:-3}
circuit_breaker_auto_reset = ${BUZZ_HEALTH_CB_AUTO_RESET:-true}
process_restart_cooldown_secs = ${BUZZ_HEALTH_RESTART_COOLDOWN:-300}

[logging]
level = "$(escape_toml "${BUZZ_LOG_LEVEL:-info}")"
format = "$(escape_toml "${BUZZ_LOG_FORMAT:-json}")"
output = "$(escape_toml "${BUZZ_LOG_OUTPUT:-stdout}")"
file_path = "$(escape_toml "${BUZZ_LOG_FILE:-/var/log/buzz-agent/buzz-agent.log}")"
max_file_size_mb = ${BUZZ_LOG_MAX_FILE_SIZE_MB:-100}
max_files = ${BUZZ_LOG_MAX_FILES:-10}
TOMLEOF
}

main() {
    log_info "Migrating configuration from $CONFIG_DIR to $OUTPUT_FILE"
    
    if [[ -f "$OUTPUT_FILE" ]]; then
        log_warn "Output file $OUTPUT_FILE already exists. Backing up..."
        cp "$OUTPUT_FILE" "$OUTPUT_FILE.bak.$(date +%s)"
    fi
    
    umask 077
    generate_toml > "$OUTPUT_FILE"
    chmod 600 "$OUTPUT_FILE"
    
    log_success "Configuration migrated to $OUTPUT_FILE"
    log_info "Review the generated file and adjust as needed"
}

main "$@"
