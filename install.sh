#!/usr/bin/env bash
# Compila e instala el indicador para el usuario actual y lo añade al inicio de sesión.
set -euo pipefail
cd "$(dirname "$0")"

cargo build --release
install -Dm755 target/release/claude-usage-indicator "$HOME/.local/bin/claude-usage-indicator"
install -Dm644 data/claude-usage-indicator.desktop "$HOME/.config/autostart/claude-usage-indicator.desktop"
sed -i "s|^Exec=.*|Exec=$HOME/.local/bin/claude-usage-indicator|" \
    "$HOME/.config/autostart/claude-usage-indicator.desktop"

pkill -x claude-usage-in || true
nohup "$HOME/.local/bin/claude-usage-indicator" >/dev/null 2>&1 &
echo "Instalado. Se iniciará automáticamente al entrar en la sesión."
