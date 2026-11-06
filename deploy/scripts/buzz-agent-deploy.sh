#!/bin/bash
# Buzz Agent Deployment Script
# Installs systemd service, config files, and sets up directories

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"

# Default paths
INSTALL_PREFIX="${INSTALL_PREFIX:-/opt/buzz-agent}"
CONFIG_DIR="${CONFIG_DIR:-/etc/buzz-agent}"
LOG_DIR="${LOG_DIR:-/var/log/buzz-agent}"
RUN_DIR="${RUN_DIR:-/var/run/buzz-agent}"
USER="${BUZZ_USER:-buzz}"
GROUP="${BUZZ_GROUP:-buzz}"
BINARY_PATH="${BINARY_PATH:-$PROJECT_ROOT/target/release/buzz-agent}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root (use sudo)"
        exit 1
    fi
}

create_user() {
    if ! id "$USER" &>/dev/null; then
        log_info "Creating user: $USER"
        useradd --system --home-dir "$INSTALL_PREFIX" --shell /bin/false "$USER"
    else
        log_info "User $USER already exists"
    fi
}

create_directories() {
    log_info "Creating directories..."
    mkdir -p "$INSTALL_PREFIX"
    mkdir -p "$CONFIG_DIR"
    mkdir -p "$LOG_DIR"
    mkdir -p "$RUN_DIR"
    mkdir -p "$INSTALL_PREFIX/.buzz/WORK_LOGS"
    
    chown -R "$USER:$GROUP" "$INSTALL_PREFIX"
    chown -R "$USER:$GROUP" "$LOG_DIR"
    chown -R "$USER:$GROUP" "$RUN_DIR"
    chmod 750 "$CONFIG_DIR"
}

install_binary() {
    log_info "Installing buzz-agent binary..."
    
    if [[ ! -f "$BINARY_PATH" ]]; then
        log_error "Binary not found at $BINARY_PATH"
        log_info "Build it first with: cargo build --release -p buzz-agent"
        exit 1
    fi
    
    cp "$BINARY_PATH" /usr/local/bin/buzz-agent
    chmod 755 /usr/local/bin/buzz-agent
    log_success "Binary installed to /usr/local/bin/buzz-agent"
}

install_systemd_service() {
    log_info "Installing systemd service..."
    
    cp "$PROJECT_ROOT/deploy/systemd/buzz-agent.service" /etc/systemd/system/buzz-agent.service
    
    # Update service file paths if needed
    sed -i "s|/opt/buzz-agent|$INSTALL_PREFIX|g" /etc/systemd/system/buzz-agent.service
    sed -i "s|User=buzz|User=$USER|g" /etc/systemd/system/buzz-agent.service
    sed -i "s|Group=buzz|Group=$GROUP|g" /etc/systemd/system/buzz-agent.service
    
    systemctl daemon-reload
    log_success "Systemd service installed"
}

install_config_templates() {
    log_info "Installing config templates..."
    
    cp "$PROJECT_ROOT/deploy/config/buzz-agent.env.example" "$CONFIG_DIR/buzz-agent.env.example"
    cp "$PROJECT_ROOT/deploy/config/health-guardian.env.example" "$CONFIG_DIR/health-guardian.env.example"
    cp "$PROJECT_ROOT/deploy/config/buzz-agent.toml.example" "$CONFIG_DIR/buzz-agent.toml.example"
    
    # Create actual config files if they don't exist
    if [[ ! -f "$CONFIG_DIR/buzz-agent.env" ]]; then
        cp "$CONFIG_DIR/buzz-agent.env.example" "$CONFIG_DIR/buzz-agent.env"
        log_warn "Created $CONFIG_DIR/buzz-agent.env from template - PLEASE EDIT IT"
    fi
    
    if [[ ! -f "$CONFIG_DIR/health-guardian.env" ]]; then
        cp "$CONFIG_DIR/health-guardian.env.example" "$CONFIG_DIR/health-guardian.env"
        log_warn "Created $CONFIG_DIR/health-guardian.env from template - PLEASE EDIT IT"
    fi
    
    chown -R root:root "$CONFIG_DIR"
    chmod 640 "$CONFIG_DIR"/*.env 2>/dev/null || true
    log_success "Config templates installed"
}

install_scripts() {
    log_info "Installing management scripts..."
    
    cp "$PROJECT_ROOT/deploy/scripts/buzz-agent-start.sh" /usr/local/bin/buzz-agent-start
    cp "$PROJECT_ROOT/deploy/scripts/buzz-agent-stop.sh" /usr/local/bin/buzz-agent-stop
    cp "$PROJECT_ROOT/deploy/scripts/buzz-agent-config-validate.sh" /usr/local/bin/buzz-agent-config-validate
    
    chmod 755 /usr/local/bin/buzz-agent-start
    chmod 755 /usr/local/bin/buzz-agent-stop
    chmod 755 /usr/local/bin/buzz-agent-config-validate
    
    log_success "Management scripts installed"
}

main() {
    log_info "Deploying Buzz Agent with System Health Guardian"
    log_info "Install prefix: $INSTALL_PREFIX"
    log_info "Config dir: $CONFIG_DIR"
    log_info "Log dir: $LOG_DIR"
    log_info "Run dir: $RUN_DIR"
    log_info "User: $USER"
    
    check_root
    create_user
    create_directories
    install_binary
    install_systemd_service
    install_config_templates
    install_scripts
    
    log_success "Deployment complete!"
    echo ""
    log_info "Next steps:"
    log_info "1. Edit $CONFIG_DIR/buzz-agent.env with your provider credentials"
    log_info "2. Edit $CONFIG_DIR/health-guardian.env to enable Health Guardian"
    log_info "3. Run: buzz-agent-config-validate"
    log_info "4. Start service: systemctl start buzz-agent"
    log_info "5. Enable on boot: systemctl enable buzz-agent"
    log_info ""
    log_info "Health Dashboard: buzz-agent health-dashboard --port 8080"
    log_info "Manual health check: buzz-agent health-check [--json]"
}

main "$@"
