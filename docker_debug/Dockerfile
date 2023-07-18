FROM ghcr.io/rust-lang/rust:nightly-alpine3.17 as builder

RUN apk update \
    && apk --no-cache --update add build-base protobuf-dev libressl-dev fuse fuse-dev pkgconfig

WORKDIR /usr/src/fairy

#ADD . .
#ENV RUSTFLAGS="-C target-feature=-crt-static"

#RUN cargo install --path /usr/src/fairy/worker
