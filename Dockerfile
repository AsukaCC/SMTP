FROM rust:1.91.1-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
COPY emails ./emails
COPY static ./static
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 bunmail && useradd --uid 10001 --gid 10001 --no-create-home bunmail
WORKDIR /app
COPY --from=build /build/target/release/bunmail-rs /usr/local/bin/bunmail-rs
COPY LICENSE THIRD_PARTY_NOTICES.md /usr/share/doc/bunmail-rs/
USER 10001:10001
EXPOSE 3000 2525
HEALTHCHECK --interval=15s --timeout=5s --start-period=30s --retries=5 CMD ["bunmail-rs", "--healthcheck"]
CMD ["bunmail-rs"]
