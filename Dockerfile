FROM rust:1.88.0-bookworm AS builder
WORKDIR /src
COPY . .
RUN cargo build --locked --release --bin algo

FROM debian:bookworm-slim
RUN useradd --create-home --uid 10001 alchetron
COPY --from=builder /src/target/release/algo /usr/local/bin/algo
COPY --from=builder /src/web /opt/alchetron/web
COPY --from=builder /src/registry /opt/alchetron/registry
COPY --from=builder /src/schemas /opt/alchetron/schemas
COPY --from=builder /src/agents /opt/alchetron/agents
COPY --from=builder /src/fixtures /opt/alchetron/fixtures
COPY --from=builder /src/eval /opt/alchetron/eval
COPY --from=builder /src/sql /opt/alchetron/sql
RUN mkdir -p /data && chown -R alchetron:alchetron /data /opt/alchetron
USER alchetron
WORKDIR /opt/alchetron
EXPOSE 8791
VOLUME ["/data"]
ENTRYPOINT ["algo", "--root", "/opt/alchetron", "--data-dir", "/data"]
CMD ["ui", "--bind", "0.0.0.0:8791", "--allow-remote", "--no-open"]
