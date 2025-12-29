# Start from the official Rust image for building
FROM rust:1.91.1 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

# Use a minimal base image for running
FROM debian:bookworm-slim
WORKDIR /app
# Copy the built binary from the builder stage
COPY --from=builder /app/target/release/notes /app/notes
# Copy any static assets or config files if needed
# COPY static/ /app/static/
# COPY config.toml /app/config.toml

# Set environment variables if needed
ENV RUST_LOG=info

# Expose the port your app listens on (e.g., 3000)
EXPOSE 3000

# Run the binary
CMD ["/app/notes"]
