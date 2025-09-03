# QLite

A lightweight, SQS-compatible message queue backed by SQLite, written in Rust.
The Primary purpose of this tool is to easily facilitate local or cicd based
testing this project is not meant for production workloads and it is not recommended
to use it for them.

## Features

- **SQS-Compatible API**: Drop-in replacement for Amazon SQS with standard operations
- **SQLite Backend**: Persistent message storage with no external dependencies
- **Web UI**: Optional dashboard for queue monitoring and message browsing
- **CLI Interface**: Command-line tools for queue management

### Web UI
![QLite Dashboard](https://github.com/stvnksslr/qlite/blob/main/docs/screenshots/dashboard.png)

## Quick Start

### Installation

```bash
cargo build --release
```

### Start Server

```bash
# Basic server on port 3000
./qlite server --port 3000

# With web UI enabled
./qlite server --port 3000 --enable-ui
```

### CLI Usage

```bash
# Create a queue
./qlite create-queue my-queue

# Send a message
./qlite send my-queue "Hello, World!"

# Receive messages
./qlite receive my-queue

# Delete a message
./qlite delete my-queue <receipt-handle>
```

## SQS API Compatibility

QLite implements the SQS Query API endpoints:

- `CreateQueue`
- `ListQueues`
- `SendMessage`
- `ReceiveMessage`
- `DeleteMessage`
- `GetQueueAttributes`

### Example with AWS CLI

```bash
# Configure AWS CLI to point to QLite
aws configure set aws_access_key_id dummy
aws configure set aws_secret_access_key dummy
aws configure set region us-east-1

# Create queue
aws sqs create-queue --endpoint-url http://localhost:3000 --queue-name test-queue

# Send message
aws sqs send-message --endpoint-url http://localhost:3000 --queue-url http://localhost:3000/test-queue --message-body "Hello from AWS CLI"
```
