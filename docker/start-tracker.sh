#!/bin/bash
# Start the anime tracker after Transmission is up

echo "Starting anime tracker..."

# Wait a moment for Transmission to fully initialize
sleep 5

# Change to app directory (for templates/assets paths)
cd /app

# Start the anime tracker
exec /app/anime-tracker
