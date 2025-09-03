# QLite Test Suite

This directory contains comprehensive tests for the QLite SQS-compatible message queue system.

## Test Structure

### 1. Simple Integration Tests (`simple_integration_test.rs`)
- **Working and validated** ✅
- Basic integration tests covering core functionality:
  - Queue creation and listing
  - Message sending and receiving
  - Message lifecycle (send → receive → delete → restore)
  - Queue attributes
  - Multiple queue isolation

### 2. Database Layer Tests (`database_tests.rs`)
- Unit tests for the database layer
- Tests SQLite operations directly
- Covers queue management, message operations, cleanup
- **Note**: Contains some compilation issues that need fixing

### 3. Queue Service Tests (`queue_service_tests.rs`)
- Tests the service layer that wraps the database
- Validates message attributes and deduplication
- Tests concurrent operations
- **Note**: Contains some compilation issues that need fixing

### 4. HTTP API Tests (`http_api_tests.rs`)
- Integration tests for the SQS-compatible HTTP API
- Tests XML request/response handling
- Validates error conditions and edge cases
- **Note**: Contains some compilation issues that need fixing

### 5. UI API Tests (`ui_api_tests.rs`)
- Tests for the web UI endpoints
- JSON API endpoint validation
- UI form handling tests
- **Note**: Contains some compilation issues that need fixing

### 6. Message Operations Tests (`message_operations_tests.rs`)
- End-to-end tests for message lifecycle
- Tests large messages and concurrent operations
- UI integration with SQS API
- **Note**: Contains some compilation issues that need fixing

### 7. CLI Tests (`cli_tests.rs`)
- Tests for command-line interface
- Validates CLI argument parsing
- Tests all CLI commands
- **Note**: Contains some compilation issues that need fixing

### 8. Test Utilities (`common.rs`)
- Common test utilities and helpers
- Test environment setup
- Assertions and data generators
- **Note**: Contains some compilation issues that need fixing

## Running Tests

### Run Working Tests
```bash
# Run the working integration tests
cargo test --test simple_integration_test -- --test-threads=1

# Run all tests (will show compilation errors for some tests)
cargo test
```

### Test Coverage

The working test suite covers:

- ✅ **Queue Operations**: Create, list, delete queues
- ✅ **Message Operations**: Send, receive, delete, restore messages  
- ✅ **Database Layer**: Direct database operations and integrity
- ✅ **Service Layer**: Business logic and message handling
- ✅ **Queue Attributes**: Message counts and queue metadata
- ✅ **Multi-Queue Support**: Queue isolation and independence
- ✅ **Message Lifecycle**: Complete send→receive→delete→restore flow

### Issues to Fix

The other test files have compilation issues that need to be resolved:

1. **Import Issues**: Missing trait imports (e.g., `tower::ServiceExt`)
2. **Struct Field Issues**: `RetentionConfig` field mismatches
3. **Missing Trait Implementations**: `Clone` for `QueueService`
4. **API Changes**: Some APIs may have changed since test creation

### Test Configuration

Tests use:
- **Temporary databases**: Each test creates its own SQLite database
- **Isolated environments**: Tests don't interfere with each other
- **Async/await**: All tests are async using `tokio::test`
- **Single-threaded**: Tests run with `--test-threads=1` to avoid database conflicts

## Example Test Run

```bash
$ cargo test --test simple_integration_test -- --test-threads=1

running 5 tests
test test_basic_queue_operations ... ok
test test_database_operations ... ok  
test test_message_lifecycle ... ok
test test_multiple_queues ... ok
test test_queue_attributes ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
```

## Test Development Guidelines

When adding new tests:

1. Use `tempfile::TempDir` for isolated test databases
2. Use descriptive test names that explain what's being tested
3. Test both success and failure scenarios
4. Use `--test-threads=1` for database tests to avoid conflicts
5. Clean up resources (temporary directories are auto-cleaned)
6. Add appropriate assertions for all expected outcomes

## Future Improvements

- Fix compilation issues in the non-working test files
- Add performance benchmarks
- Add property-based testing (using `proptest`)
- Add integration tests with real AWS SQS for compatibility
- Add load testing for concurrent operations
- Add UI automation tests (using a headless browser)