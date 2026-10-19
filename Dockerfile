# Image du serveur. Par défaut en stdio (clients MCP locaux, registre MCP) ;
# pour le mode HTTP : -e ENT1_TRANSPORT=http -e ENT1_ECOUTE=0.0.0.0:8080 -p 8080:8080.
FROM rust:1-slim-bookworm AS construction
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM gcr.io/distroless/cc-debian12:nonroot
LABEL io.modelcontextprotocol.server.name="io.github.edouard-claude/ent1-mcp" \
      org.opencontainers.image.source="https://github.com/edouard-claude/ent1-mcp" \
      org.opencontainers.image.description="Serveur MCP pour l'ENT ONE (Edifice)" \
      org.opencontainers.image.licenses="MIT OR Apache-2.0"
COPY --from=construction /src/target/release/ent1-mcp /usr/local/bin/ent1-mcp
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/ent1-mcp"]
