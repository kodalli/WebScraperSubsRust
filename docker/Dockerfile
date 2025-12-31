# Stage 1: Build the Rust application
FROM rust:1.83-slim-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY templates ./templates
COPY assets ./assets

RUN cargo build --release

# Stage 2: Final image based on transmission-openvpn
FROM haugene/transmission-openvpn:latest

# Copy the compiled Rust binary
COPY --from=builder /app/target/release/web_scraper_subs_rust /app/anime-tracker
COPY --from=builder /app/templates /app/templates
COPY --from=builder /app/assets /app/assets

# Create data directories
RUN mkdir -p /app/data

# Copy custom startup script
COPY docker/start-tracker.sh /etc/openvpn/start-tracker.sh
RUN chmod +x /etc/openvpn/start-tracker.sh

# Environment variables for the anime tracker
ENV DATABASE_PATH=/app/data/tracker.db
ENV TRANSMISSION_HOST=localhost
ENV TRANSMISSION_PORT=9091
ENV PORT=8080

# Expose both Transmission and anime tracker ports
EXPOSE 9091 8080

# The base image handles the entrypoint - we hook into tunnelUp.sh
# by adding our script to run after Transmission starts
RUN echo '/etc/openvpn/start-tracker.sh &' >> /etc/openvpn/tunnelUp.sh
