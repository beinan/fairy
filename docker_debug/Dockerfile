FROM ghcr.io/rust-lang/rust:nightly-alpine3.17 as builder

RUN apk update \
    && apk --no-cache --update add build-base protobuf-dev libressl-dev fuse fuse-dev

WORKDIR /usr/src/fairy

#ADD . .
#
#RUN cargo install --path /usr/src/fairy

