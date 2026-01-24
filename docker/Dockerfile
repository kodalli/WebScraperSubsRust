# Stage 1: Build TailwindCSS styles
FROM node:20-slim AS styles-builder

WORKDIR /app
COPY package.json package-lock.json ./
COPY tailwind.config.cjs ./
COPY styles ./styles
COPY templates ./templates

RUN npm ci
RUN mkdir -p assets && npx tailwindcss -i ./styles/tailwindcss.css -o ./assets/main.css --minify

# Stage 2: Cache Rust dependencies with dummy project
FROM rust:1.85-slim-bookworm AS deps-builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy ONLY manifests, create dummy src
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs

# Build deps only (cached until Cargo.toml/lock change)
RUN cargo build --release && \
    rm -rf src target/release/deps/web_scraper_subs_rust* \
           target/release/web_scraper_subs_rust* \
           target/release/.fingerprint/web_scraper_subs_rust*

# Stage 3: Build actual app (inherits cached deps)
FROM deps-builder AS builder
COPY src ./src
RUN cargo build --release && \
    strip --strip-all /app/target/release/web_scraper_subs_rust

# Stage 4: Final image - pinned version for reproducibility
FROM haugene/transmission-openvpn:5.3.1

# Copy the compiled Rust binary
COPY --from=builder /app/target/release/web_scraper_subs_rust /app/anime-tracker

# Copy templates and assets directly from build context (not builder stage)
COPY templates /app/templates
COPY --from=styles-builder /app/assets/main.css /app/assets/main.css
COPY assets /app/assets

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
# Insert our script before the 'exit 0' line
RUN sed -i 's|^exit 0|/etc/openvpn/start-tracker.sh \&\nexit 0|' /etc/openvpn/tunnelUp.sh
