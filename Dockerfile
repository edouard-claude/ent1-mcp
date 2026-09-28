# Image du serveur en mode HTTP.
FROM rust:1-slim AS construction
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=construction /src/target/release/ent1-mcp /usr/local/bin/ent1-mcp
ENV ENT1_TRANSPORT=http \
    ENT1_ECOUTE=0.0.0.0:8080
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/ent1-mcp"]
