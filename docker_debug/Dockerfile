FROM ghcr.io/rust-lang/rust:nightly-alpine3.17 as builder

RUN apk update \
    && apk --no-cache --update add build-base protobuf-dev libressl-dev fuse fuse-dev cmake clang git pkgconfig clang-dev

WORKDIR /usr/src/fairy

#ADD . .
#
EVN RUSTFLAGS="-C target-feature=-crt-static"
RUN cargo install --path /usr/src/fairy/worker
