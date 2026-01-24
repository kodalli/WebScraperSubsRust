#!/bin/bash
# Start the anime tracker after Transmission is up

echo "Starting anime tracker..."

# Wait a moment for Transmission to fully initialize
sleep 5

# ========================================
# Fix routing for incoming connections
# ========================================
# Get container's eth0 IP and Docker gateway
ETH0_IP=$(ip -4 addr show eth0 | grep -oP '(?<=inet\s)\d+(\.\d+){3}')
DOCKER_GW=$(ip route | grep "default via" | grep eth0 | awk '{print $3}')

echo "eth0 IP: $ETH0_IP, Docker gateway: $DOCKER_GW"

# Method: Source-based policy routing
# Route all traffic originating FROM our eth0 IP back through eth0
if [ -n "$ETH0_IP" ] && [ -n "$DOCKER_GW" ]; then
    # Add route table for local network responses
    ip route add default via $DOCKER_GW table 100 2>/dev/null || true

    # Policy: traffic from our container IP uses table 100 (via eth0)
    ip rule add from $ETH0_IP table 100 priority 100 2>/dev/null || true

    echo "Routing fix applied: $ETH0_IP -> table 100 -> $DOCKER_GW"
else
    echo "WARNING: Could not determine network configuration"
fi
# ========================================

# Change to app directory (for templates/assets paths)
cd /app

# Export environment variables (may not be inherited in openvpn script context)
export PORT="${PORT:-8080}"
export DATABASE_PATH="${DATABASE_PATH:-/app/data/tracker.db}"
export TRANSMISSION_HOST="${TRANSMISSION_HOST:-localhost}"
export TRANSMISSION_PORT="${TRANSMISSION_PORT:-9091}"

# Start the anime tracker
exec /app/anime-tracker
