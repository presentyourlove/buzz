#!/bin/bash
# Buzz Agent Configuration Validation Script
# Validates environment variables and TOML config files

set -euo pipefail

CONFIG_DIR="${1:-/etc/buzz-agent}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

ERRORS=0
WARNINGS=0

check_var() {
    local var_name="$1"
    local var_value="${!var_name:-}"
    local required="${2:-false}"
    
    if [[ -z "$var_value" ]]; then
        if [[ "$required" == "true" ]]; then
            log_error "Required variable $var_name is not set"
            ((ERRORS++))
        else
            log_warn "Optional variable $var_name is not set"
            ((WARNINGS++))
        fi
    else
        log_success "$var_name is set"
    fi
}

validate_provider_config() {
    local provider="${BUZZ_AGENT_PROVIDER:-}"
    
    if [[ -z "$provider" ]]; then
        log_error "BUZZ_AGENT_PROVIDER is required"
        ((ERRORS++))
        return
    fi
    
    case "$provider" in
        anthropic)
            check_var "ANTHROPIC_API_KEY" true
            ;;
        openai)
            check_var "OPENAI_COMPAT_API_KEY" true
            ;;
        databricks|databricks_v2)
            check_var "DATABRICKS_HOST" true
            check_var "DATABRICKS_MODEL" true
            ;;
        openrouter)
            check_var "OPENROUTER_API_KEY" true
            check_var "OPENROUTER_MODEL" true
            ;;
        *)
            log_error "Unknown provider: $provider"
            ((ERRORS++))
            ;;
    esac
}

validate_health_guardian() {
    if [[ "${BUZZ_HEALTH_GUARDIAN_ENABLED:-0}" != "1" ]]; then
        log_info "Health Guardian is disabled"
        return
    fi
    
    log_info "Validating Health Guardian configuration..."
    
    check_var "BUZZ_HEALTH_GUARDIAN_CHANNEL" true
    check_var "BUZZ_PRIVATE_KEY" true
    check_var "BUZZ_RELAY_URL" false
    
    # Validate cron expressions
    local schedule="${BUZZ_HEALTH_GUARDIAN_SCHEDULE:-*/15 * * * *}"
    local daily_deep="${BUZZ_HEALTH_GUARDIAN_DAILY_DEEP:-0 4 * * *}"
    
    if ! echo "$schedule" | grep -qE '^([\*/0-9,-]+ +){4}[\*/0-9,-]+$'; then
        log_warn "BUZZ_HEALTH_GUARDIAN_SCHEDULE may be invalid: $schedule"
        ((WARNINGS++))
    fi
    
    if ! echo "$daily_deep" | grep -qE '^([\*/0-9,-]+ +){4}[\*/0-9,-]+$'; then
        log_warn "BUZZ_HEALTH_GUARDIAN_DAILY_DEEP may be invalid: $daily_deep"
        ((WARNINGS++))
    fi
    
    # Validate paths
    local work_logs="${BUZZ_WORK_LOGS_PATH:-./.buzz/WORK_LOGS}"
    if [[ ! -d "$work_logs" ]]; then
        log_warn "WORK_LOGS path does not exist: $work_logs (will be created)"
        ((WARNINGS++))
    fi
}

validate_toml_config() {
    local toml_file="$CONFIG_DIR/buzz-agent.toml"
    
    if [[ ! -f "$toml_file" ]]; then
        log_info "No TOML config file found at $toml_file (using env vars)"
        return
    fi
    
    log_info "Validating TOML config: $toml_file"
    
    # Check if toml is valid (basic syntax check)
    if command -v python3 >/dev/null 2>&1; then
        if python3 -c "import tomli; tomli.load(open('$toml_file', 'rb'))" 2>/dev/null; then
            log_success "TOML syntax is valid"
        else
            log_error "TOML syntax is invalid"
            ((ERRORS++))
        fi
    else
        log_warn "python3 not available, skipping TOML syntax check"
    fi
}

main() {
    log_info "Validating Buzz Agent configuration in $CONFIG_DIR"
    
    # Load env files
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
    
    echo ""
    log_info "=== Provider Configuration ==="
    validate_provider_config
    
    echo ""
    log_info "=== Agent Identity ==="
    check_var "BUZZ_PRIVATE_KEY" false
    check_var "BUZZ_RELAY_URL" false
    
    echo ""
    log_info "=== Health Guardian ==="
    validate_health_guardian
    
    echo ""
    log_info "=== TOML Config ==="
    validate_toml_config
    
    echo ""
    if [[ $ERRORS -eq 0 && $WARNINGS -eq 0 ]]; then
        log_success "All checks passed!"
        exit 0
    elif [[ $ERRORS -eq 0 ]]; then
        log_warn "Validation passed with $WARNINGS warning(s)"
        exit 0
    else
        log_error "Validation failed with $ERRORS error(s) and $WARNINGS warning(s)"
        exit 1
    fi
}

main "$@"
