# Conversational Agents platform (cxa) — container image for Cloud Run.
#
#   docker build -t cxa .
#   docker run -p 8080:8080 -e CXA_SEED=/app/examples/pizza-agent.json cxa
#
# The workspace has no third-party crates, so the build needs no registry
# access beyond the base images.

ARG RUST_VERSION=1

FROM rust:${RUST_VERSION}-slim-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release --locked -p cxa \
 && strip target/release/cxa \
 && mkdir -p /data

# Distroless: glibc + CA certificates, no shell, runs as uid 65532.
FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=build /src/target/release/cxa /app/cxa
COPY examples /app/examples
COPY --from=build --chown=65532:65532 /data /data

# Cloud Run sets PORT; cxa listens on 0.0.0.0:$PORT when it is present.
# /data is ephemeral unless a volume (e.g. a Cloud Storage bucket) is
# mounted there — see docs/deploy-cloud-run.md.
ENV PORT=8080 \
    CXA_DATA=/data/cxa-state.json
EXPOSE 8080
USER nonroot:nonroot
ENTRYPOINT ["/app/cxa", "serve"]
