FROM ghcr.io/rust-lang/rust:nightly-alpine3.17 as builder

RUN apk update \
    && apk --no-cache --update add build-base protobuf-dev libressl-dev

WORKDIR /usr/src/fairy
COPY . .
RUN cargo install --path .

FROM debian:buster-slim
RUN apt-get update & apt-get install -y extra-runtime-dependencies & rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/fairy /usr/local/bin/fairy

CMD ["fairy"]