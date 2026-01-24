#!/bin/bash
# Start the anime tracker after Transmission is up

echo "Starting anime tracker..."

# Wait a moment for Transmission to fully initialize
sleep 5

# Change to app directory (for templates/assets paths)
cd /app

# Export environment variables (may not be inherited in openvpn script context)
export PORT="${PORT:-8080}"
export DATABASE_PATH="${DATABASE_PATH:-/app/data/tracker.db}"
export TRANSMISSION_HOST="${TRANSMISSION_HOST:-localhost}"
export TRANSMISSION_PORT="${TRANSMISSION_PORT:-9091}"

# Start the anime tracker
exec /app/anime-tracker
