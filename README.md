# Fairy: A Distributed Cache in Rust

[![Build Status](https://travis-ci.com/beinan/fairy.svg?branch=master)](https://travis-ci.com/beinan/fairy)

Fairy is a distributed cache implemented in Rust. It uses consistent hashing and a combination of memory and ssd to store and manage key-value pairs across multiple nodes in a network.

## Features

- Distributed caching with consistent hashing
- Lock-free eviction policy
- Supports adding and removing nodes dynamically
- Fault-tolerant with automatic failover

## Getting Started

### Prerequisites

- Rust nightly toolchain
- Cargo

#### Build in MacOS
You might need
```ssh
brew install pkg-config
brew install macfuse
```

#### Build and debug with docker
Build the debug container
```ssh
docker build -t fairy-debug -f docker_debug/Dockerfile .
```

Start a debug container
```ssh
docker run -it --rm -v ${PWD}:/usr/src/fairy fairy-debug
```

You might need more permissions if you want to try POSIX/fuse interface
```ssh
docker run -it --rm --cap-add SYS_ADMIN --device /dev/fuse --security-opt apparmor:unconfined -v ${PWD}:/usr/src/fairy fairy-debug
```

### Installation

Add the following to your `Cargo.toml` file:

```toml
[dependencies]
fairy = { git = "https://github.com/beinan/fairy.git", branch = "master" }
