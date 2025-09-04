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
![QLite Dashboard](docs/screenshots/dashboard.png)

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

### Quick Start with AWS CLI

```bash
# Set dummy credentials (any values work)
export AWS_ACCESS_KEY_ID=dummy
export AWS_SECRET_ACCESS_KEY=dummy

# Start QLite server
cargo run -- server --port 3000

# Use AWS CLI exactly like with real SQS
aws sqs create-queue --endpoint-url http://localhost:3000 --queue-name test-queue
aws sqs list-queues --endpoint-url http://localhost:3000
aws sqs send-message --endpoint-url http://localhost:3000 --queue-url http://localhost:3000/test-queue --message-body "Hello World"
aws sqs receive-message --endpoint-url http://localhost:3000 --queue-url http://localhost:3000/test-queue
```

### SDK Integration Example

```python
# Python boto3 example
import boto3

sqs = boto3.client(
    'sqs',
    endpoint_url='http://localhost:3000',
    aws_access_key_id='dummy',
    aws_secret_access_key='dummy',
    region_name='us-east-1'
)

# Works exactly like AWS SQS
queue = sqs.create_queue(QueueName='my-test-queue')
sqs.send_message(QueueUrl=queue['QueueUrl'], MessageBody='Hello from Python!')
```

## SQS Feature Compatibility Matrix

This matrix shows which AWS SQS features are supported by QLite, available in AWS SQS, and covered by our test suite.

| Feature | QLite Support | AWS SQS | Tested |
|---------|:-------------:|:-------:|:------:|
| **Core Queue Operations** |
| CreateQueue | ✅ | ✅ | ✅ |
| ListQueues | ✅ | ✅ | ✅ |
| GetQueueUrl | ✅ | ✅ | ✅ |
| DeleteQueue | ✅ | ✅ | ✅ |
| GetQueueAttributes | ✅ | ✅ | ✅ |
| SetQueueAttributes | ✅ | ✅ | ✅ |
| **Message Operations** |
| SendMessage | ✅ | ✅ | ✅ |
| ReceiveMessage | ✅ | ✅ | ✅ |
| DeleteMessage | ✅ | ✅ | ✅ |
| SendMessageBatch | ✅ | ✅ | ✅ |
| DeleteMessageBatch | ✅ | ✅ | ✅ |
| **Message Attributes** |
| MessageAttributes | ✅ | ✅ | ✅ |
| MessageSystemAttributes | ✅ | ✅ | ✅ |
| **Queue Types** |
| Standard Queues | ✅ | ✅ | ✅ |
| FIFO Queues (.fifo) | ✅ | ✅ | ✅ |
| **Queue Attributes** |
| VisibilityTimeout | ✅ | ✅ | ✅ |
| MessageRetentionPeriod | ✅ | ✅ | ✅ |
| DelaySeconds | ✅ | ✅ | ✅ |
| MaxReceiveCount | ✅ | ✅ | ✅ |
| RedrivePolicy (DLQ) | ✅ | ✅ | ✅ |
| **FIFO-Specific Features** |
| MessageGroupId | ✅ | ✅ | ✅ |
| MessageDeduplicationId | ✅ | ✅ | ✅ |
| ContentBasedDeduplication | ✅ | ✅ | ✅ |
| FifoThroughputLimit | ❌ | ✅ | ❌ |
| DeduplicationScope | ❌ | ✅ | ❌ |
| **Advanced Features** |
| Long Polling (WaitTimeSeconds) | ✅ | ✅ | ✅ |
| Short Polling | ✅ | ✅ | ✅ |
| Message Timers (DelaySeconds) | ✅ | ✅ | ✅ |
| Dead Letter Queues | ✅ | ✅ | ✅ |
| **Security & Access** |
| IAM Integration | ❌ | ✅ | ❌ |
| SQS Access Policy | ❌ | ✅ | ❌ |
| Server-Side Encryption | ❌ | ✅ | ❌ |
| **Monitoring & Management** |
| CloudWatch Metrics | ❌ | ✅ | ❌ |
| Message Tracing | ❌ | ✅ | ❌ |
| Tags | ❌ | ✅ | ❌ |
| **Format Support** |
| XML Responses | ✅ | ✅ | ✅ |
| JSON Responses (SDK) | ✅ | ✅ | ✅ |
| Form-encoded Requests | ✅ | ✅ | ✅ |

### Legend
- ✅ **Fully Supported/Available** - Feature works as expected
- ⚠️ **Partial/Issues** - Feature implemented but has limitations or test gaps
- ❌ **Not Supported** - Feature not implemented or not applicable

### Test Coverage
QLite includes comprehensive test scripts in the `scripts/` directory:
- `comprehensive_aws_cli_test.sh` - Full feature testing
- `production_readiness_test.sh` - Core functionality validation  
- `detailed_aws_cli_test.sh` - Detailed output testing
- `test_sqs_compatible.sh` - Format compatibility testing
- `test_fixes.sh` - Validates all bug fixes and improvements
- `test_max_receive_count.sh` - MaxReceiveCount and Dead Letter Queue testing

Run tests with: `./scripts/production_readiness_test.sh` or `./scripts/test_fixes.sh`
