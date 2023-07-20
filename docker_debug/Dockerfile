FROM ghcr.io/rust-lang/rust:nightly-alpine3.17 as builder

RUN apk update \
    && apk --no-cache --update add build-base protobuf-dev libressl-dev fuse fuse-dev pkgconfig fio

WORKDIR /usr/src/fairy

#ADD . .
ENV RUSTFLAGS="-C target-feature=-crt-static"
ENV CARGO_HOME=/usr/src/fairy/dockerout/cargo
ENV CARGO_TARGET_DIR=/usr/src/fairy/dockerout/target

#RUN cargo install --path /usr/src/fairy/worker
