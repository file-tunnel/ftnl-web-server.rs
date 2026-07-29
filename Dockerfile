FROM rust:1.96-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY web ./web
RUN cargo build --locked --release

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/ftnl-web-server /usr/local/bin/ftnl-web-server
EXPOSE 3000
ENV FTNL_WEB_BIND=0.0.0.0:3000
ENTRYPOINT ["/usr/local/bin/ftnl-web-server"]
