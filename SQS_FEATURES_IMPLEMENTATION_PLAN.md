# QLite AWS SQS Features Implementation Plan

## Overview
This document outlines a comprehensive phased approach to implement all requested AWS SQS features in QLite. The implementation is organized into phases based on complexity and dependencies.

## Current State Analysis
- ✅ Basic SQS operations (CreateQueue, ListQueues, SendMessage, ReceiveMessage, DeleteMessage, GetQueueAttributes)
- ✅ AWS CLI/SDK JSON format support with X-Amz-Target headers
- ✅ Message attributes and basic deduplication support
- ✅ Visibility timeout and message retention
- ✅ Web UI for queue management

## Requested Features to Implement
1. GetQueueUrl
2. SetQueueAttributes 
3. DelaySeconds
4. SendMessageBatch
5. ReceiveMessageBatch
6. DeleteMessageBatch
7. FIFO Queues
8. Dead Letter Queues (complete implementation)
9. Message Groups
10. Long Polling

---

## Phase 1: Basic API Completeness (1-2 days)
**Goal**: Complete the fundamental SQS API surface

### 1.1 GetQueueUrl Implementation
- **Files to modify**: `src/http_server.rs`, `src/sqs_types.rs`
- **Changes**:
  - Add `GetQueueUrlResponse` and `GetQueueUrlResult` types
  - Add handler function that checks queue existence and returns URL
  - Add route mapping in action dispatcher

### 1.2 SetQueueAttributes Implementation  
- **Files to modify**: `src/http_server.rs`, `src/sqs_types.rs`, `src/database.rs`
- **Changes**:
  - Add `SetQueueAttributesResponse` and `SetQueueAttributesResult` types
  - Add handler to parse and store queue attributes
  - Extend database schema to store queue configuration
  - Support attributes: VisibilityTimeout, MessageRetentionPeriod, DelaySeconds

### 1.3 DelaySeconds Support
- **Files to modify**: `src/message.rs`, `src/queue_service.rs`, `src/database.rs`, `src/http_server.rs`
- **Changes**:
  - Add `delay_until` field to Message struct  
  - Update `send_message` to parse DelaySeconds parameter
  - Modify `receive_message` to respect delay timing
  - Add database column for delay_until timestamp
  - Add migration logic for existing databases

**Expected Outcome**: QLite supports queue URL lookup, attribute configuration, and message delays

---

## Phase 2: Batch Operations (2-3 days)
**Goal**: Implement batch processing for improved throughput

### 2.1 Database Schema Enhancements
- **Files to modify**: `src/database.rs`
- **Changes**:
  - Add batch insert/delete/update operations
  - Optimize queries for bulk operations
  - Add transaction support for atomic batch operations

### 2.2 SendMessageBatch Implementation
- **Files to modify**: `src/http_server.rs`, `src/sqs_types.rs`, `src/queue_service.rs`
- **Changes**:
  - Add `SendMessageBatchResponse` and related result types
  - Parse batch request format: `SendMessageBatchRequestEntry.N.*`
  - Implement batch message validation and processing
  - Handle partial failures with proper error responses
  - Support up to 10 messages per batch (AWS limit)

### 2.3 DeleteMessageBatch Implementation
- **Files to modify**: `src/http_server.rs`, `src/sqs_types.rs`, `src/queue_service.rs`
- **Changes**:
  - Add `DeleteMessageBatchResponse` and related types
  - Parse batch delete format: `DeleteMessageBatchRequestEntry.N.*`
  - Implement bulk delete operations
  - Handle receipt handle validation for batches

### 2.4 ReceiveMessageBatch Implementation
- **Files to modify**: `src/http_server.rs`, `src/sqs_types.rs`, `src/queue_service.rs`
- **Changes**:
  - Enhance ReceiveMessage to support MaxNumberOfMessages parameter
  - Return multiple messages in single response
  - Maintain individual visibility timeouts per message
  - Support up to 10 messages per batch

**Expected Outcome**: All batch operations working with proper AWS-compatible error handling

---

## Phase 3: FIFO Queue Support (3-4 days)
**Goal**: Implement First-In-First-Out message ordering

### 3.1 Queue Type Detection and Creation
- **Files to modify**: `src/message.rs`, `src/queue_service.rs`, `src/database.rs`
- **Changes**:
  - Detect FIFO queues by `.fifo` suffix
  - Add `is_fifo` flag to Queue struct
  - Create separate FIFO-specific database tables or columns
  - Add FIFO queue validation (naming, attributes)

### 3.2 Message Ordering Infrastructure  
- **Files to modify**: `src/message.rs`, `src/database.rs`, `src/queue_service.rs`
- **Changes**:
  - Add `sequence_number` field to Message struct
  - Implement sequence number generation and management
  - Add database indices for efficient FIFO ordering
  - Modify receive operations to respect FIFO order

### 3.3 Message Groups Implementation
- **Files to modify**: `src/message.rs`, `src/http_server.rs`, `src/queue_service.rs`
- **Changes**:
  - Add `message_group_id` field to Message struct
  - Parse MessageGroupId from send requests
  - Implement per-group FIFO ordering
  - Add MessageGroupId to response structures
  - Ensure exactly-once processing per message group

### 3.4 Content-Based Deduplication
- **Files to modify**: `src/queue_service.rs`, `src/database.rs`
- **Changes**:
  - Implement SHA-256 hashing of message body
  - Add deduplication logic for FIFO queues
  - Support both explicit and content-based deduplication
  - Add 5-minute deduplication window (AWS standard)

**Expected Outcome**: Full FIFO queue support with message groups and deduplication

---

## Phase 4: Dead Letter Queue Enhancement (2-3 days) 
**Goal**: Complete the existing partial DLQ implementation

### 4.1 DLQ Configuration
- **Files to modify**: `src/message.rs`, `src/database.rs`, `src/config.rs`
- **Changes**:
  - Add RedrivePolicy attribute support
  - Link queues to their DLQ via configuration
  - Add maxReceiveCount tracking per message
  - Create DLQ tables/schema if not exists

### 4.2 DLQ Message Processing
- **Files to modify**: `src/queue_service.rs`, `src/database.rs`
- **Changes**:
  - Complete `move_message_to_dlq` implementation
  - Add automatic DLQ routing on max receives exceeded
  - Implement `get_dlq_messages` functionality
  - Add DLQ message metadata (failure reason, original queue)

### 4.3 DLQ Management Operations
- **Files to modify**: `src/http_server.rs`, `src/sqs_types.rs`
- **Changes**:
  - Add `RedrivePolicyDocument` parsing
  - Implement redrive operations (move messages back to source)
  - Add DLQ purge operations
  - Add DLQ-specific queue attributes

**Expected Outcome**: Complete DLQ implementation with management operations

---

## Phase 5: Long Polling Support (1-2 days)
**Goal**: Implement efficient long polling for reduced API calls

### 5.1 Polling Infrastructure
- **Files to modify**: `src/http_server.rs`, `src/queue_service.rs`
- **Changes**:
  - Add WaitTimeSeconds parameter parsing
  - Implement async waiting for message availability  
  - Add timeout handling (up to 20 seconds)
  - Use tokio channels for efficient waiting

### 5.2 Message Notifications
- **Files to modify**: `src/queue_service.rs`, `src/database.rs`
- **Changes**:
  - Add message arrival notification system
  - Implement subscriber pattern for waiting requests
  - Add proper cleanup for timed-out polls
  - Ensure single message delivery for concurrent polls

**Expected Outcome**: Long polling reduces unnecessary API calls and improves efficiency

---

## Phase 6: Advanced Features & Polish (2-3 days)
**Goal**: Production readiness and advanced AWS compatibility

### 6.1 Enhanced Error Handling
- **Files to modify**: `src/http_server.rs`, `src/sqs_types.rs`
- **Changes**:
  - Implement all AWS SQS error codes
  - Add proper HTTP status codes for each error type
  - Improve error messages and debugging information
  - Add request validation and parameter checking

### 6.2 Performance Optimizations
- **Files to modify**: `src/database.rs`, `src/queue_service.rs`
- **Changes**:
  - Add database connection pooling
  - Optimize SQL queries for large message volumes
  - Add proper indexing for all query patterns
  - Implement message cleanup background tasks

### 6.3 Metrics and Monitoring
- **Files to modify**: `src/http_server.rs`, `src/ui.rs`
- **Changes**:
  - Add detailed queue metrics (message age, throughput)
  - Enhance web UI with FIFO and DLQ information
  - Add health check improvements
  - Add performance metrics collection

**Expected Outcome**: Production-ready implementation with comprehensive monitoring

---

## Phase 7: Testing & Documentation (1-2 days)
**Goal**: Ensure reliability and usability

### 7.1 Comprehensive Test Suite
- **Files to create/modify**: `tests/`, various test files
- **Changes**:
  - Add integration tests for all new features
  - Create AWS CLI compatibility test suite
  - Add load testing for batch operations
  - Test FIFO ordering guarantees
  - Test DLQ functionality end-to-end
  - Add long polling behavior tests

### 7.2 Documentation Updates
- **Files to modify**: `README.md`, new documentation files
- **Changes**:
  - Update compatibility table with new features
  - Add usage examples for all new features
  - Create FIFO queue usage guide
  - Document DLQ configuration and management
  - Add performance tuning guide

**Expected Outcome**: Well-tested, documented implementation ready for production use

---

## Implementation Priority Matrix

| Feature | Complexity | User Impact | AWS Compatibility | Priority |
|---------|------------|-------------|-------------------|----------|
| GetQueueUrl | Low | Medium | High | High |
| DelaySeconds | Medium | High | High | High |
| SetQueueAttributes | Medium | High | High | High |
| SendMessageBatch | High | High | High | Medium |
| DeleteMessageBatch | Medium | Medium | High | Medium |
| ReceiveMessageBatch | Low | Medium | High | Medium |
| FIFO Queues | Very High | Medium | Medium | Low |
| Message Groups | High | Low | Medium | Low |
| Dead Letter Queues | High | Medium | Medium | Medium |
| Long Polling | Medium | Low | Medium | Low |

## Database Migration Strategy

### Migration 1: Basic Extensions
```sql
-- Add new columns to existing tables
ALTER TABLE messages ADD COLUMN delay_until TEXT;
ALTER TABLE messages ADD COLUMN message_group_id TEXT;
ALTER TABLE messages ADD COLUMN sequence_number INTEGER;

-- Add queue configuration table
CREATE TABLE queue_config (
  name TEXT PRIMARY KEY,
  is_fifo BOOLEAN DEFAULT FALSE,
  content_based_deduplication BOOLEAN DEFAULT FALSE,
  visibility_timeout_seconds INTEGER DEFAULT 30,
  message_retention_period_seconds INTEGER DEFAULT 345600,
  max_receive_count INTEGER,
  dead_letter_target_arn TEXT
);
```

### Migration 2: FIFO Support
```sql
-- Add indexes for FIFO ordering
CREATE INDEX idx_messages_fifo_order ON messages(queue_name, message_group_id, sequence_number);
CREATE INDEX idx_messages_delay ON messages(queue_name, delay_until);
```

### Migration 3: DLQ Tables
```sql  
-- Dead letter queue messages
CREATE TABLE dead_letter_messages (
  id TEXT PRIMARY KEY,
  original_queue_name TEXT NOT NULL,
  dlq_name TEXT NOT NULL,
  failure_reason TEXT NOT NULL,
  moved_at TEXT NOT NULL,
  original_message_data TEXT NOT NULL
);
```

## Risk Assessment and Mitigation

### High Risk Items
1. **Database Schema Changes**: Risk of data loss during migrations
   - **Mitigation**: Comprehensive backup strategy and rollback procedures

2. **FIFO Implementation Complexity**: Risk of message ordering bugs
   - **Mitigation**: Extensive testing with concurrent scenarios

3. **Performance Impact**: New features may slow existing operations  
   - **Mitigation**: Benchmarking and optimization during development

### Medium Risk Items
1. **Batch Operation Edge Cases**: Complex error handling scenarios
2. **Long Polling Resource Usage**: Potential memory leaks with many connections
3. **AWS Compatibility**: Subtle differences from real SQS behavior

## Success Metrics

### Functional Metrics
- [ ] All 10 requested features implemented and tested
- [ ] 95%+ compatibility with AWS SQS API
- [ ] Zero breaking changes to existing functionality
- [ ] Complete test coverage for new features

### Performance Metrics  
- [ ] Batch operations 5x faster than individual operations
- [ ] FIFO queues maintain < 1ms ordering guarantees
- [ ] Long polling reduces API calls by 80%
- [ ] No performance regression for existing operations

### Quality Metrics
- [ ] Comprehensive error handling with proper AWS error codes
- [ ] Complete documentation and usage examples
- [ ] Production-ready monitoring and metrics
- [ ] Clean, maintainable code structure

---

## Estimated Timeline: 12-18 development days

This plan provides a structured approach to implementing all requested AWS SQS features while maintaining system stability and ensuring thorough testing. Each phase builds upon the previous one, allowing for incremental delivery and validation.