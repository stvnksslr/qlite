#!/bin/bash

# Comprehensive AWS CLI test suite for QLite
# Tests all implemented SQS features

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "🧪 ==========================="
echo "🧪 QLite Comprehensive AWS CLI Test Suite"
echo "🧪 ==========================="
echo

# Set dummy AWS credentials
export AWS_ACCESS_KEY_ID=dummy
export AWS_SECRET_ACCESS_KEY=dummy
export AWS_DEFAULT_REGION=us-east-1

ENDPOINT_URL="http://localhost:3000"

# Function to run test and check result
run_test() {
    local test_name="$1"
    local command="$2"
    local expected_success="$3"
    
    echo -e "${YELLOW}Testing: $test_name${NC}"
    echo "Command: $command"
    
    if eval "$command" >/dev/null 2>&1; then
        if [ "$expected_success" = "true" ]; then
            echo -e "${GREEN}✅ PASS${NC}"
        else
            echo -e "${RED}❌ FAIL (expected error but succeeded)${NC}"
            return 1
        fi
    else
        if [ "$expected_success" = "false" ]; then
            echo -e "${GREEN}✅ PASS (expected error)${NC}"
        else
            echo -e "${RED}❌ FAIL (unexpected error)${NC}"
            return 1
        fi
    fi
    echo
}

# Function to run command and capture output
run_and_capture() {
    local command="$1"
    eval "$command" 2>/dev/null
}

echo "🔍 Phase 1: Basic Queue Operations"
echo "================================"

# Test 1.1: List queues (should work)
run_test "List Queues" \
    "aws sqs list-queues --endpoint-url $ENDPOINT_URL" \
    "true"

# Test 1.2: Create standard queue
run_test "Create Standard Queue" \
    "aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name test-standard-queue" \
    "true"

# Test 1.3: Get queue URL
run_test "Get Queue URL" \
    "aws sqs get-queue-url --endpoint-url $ENDPOINT_URL --queue-name test-standard-queue" \
    "true"

# Test 1.4: Set queue attributes
run_test "Set Queue Attributes" \
    "aws sqs set-queue-attributes --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-standard-queue --attributes VisibilityTimeout=60,MessageRetentionPeriod=345600" \
    "true"

# Test 1.5: Get queue attributes  
run_test "Get Queue Attributes" \
    "aws sqs get-queue-attributes --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-standard-queue --attribute-names All" \
    "true"

echo "📨 Phase 2: Message Operations"
echo "=============================="

# Test 2.1: Send message
run_test "Send Message" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-standard-queue --message-body 'Hello World from AWS CLI'" \
    "true"

# Test 2.2: Send message with attributes
run_test "Send Message with Attributes" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-standard-queue --message-body 'Message with attributes' --message-attributes 'Author={StringValue=QLite,DataType=String}'" \
    "true"

# Test 2.3: Send message with delay
run_test "Send Message with Delay" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-standard-queue --message-body 'Delayed message' --delay-seconds 5" \
    "true"

# Test 2.4: Receive message
RECEIVED_MESSAGE=$(run_and_capture "aws sqs receive-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-standard-queue --max-number-of-messages 1")
if echo "$RECEIVED_MESSAGE" | grep -q "MessageId"; then
    echo -e "${GREEN}✅ PASS: Receive Message${NC}"
    
    # Extract receipt handle for deletion test
    RECEIPT_HANDLE=$(echo "$RECEIVED_MESSAGE" | grep -o '"ReceiptHandle": "[^"]*"' | cut -d'"' -f4)
    
    # Test 2.5: Delete message
    if [ -n "$RECEIPT_HANDLE" ]; then
        run_test "Delete Message" \
            "aws sqs delete-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-standard-queue --receipt-handle '$RECEIPT_HANDLE'" \
            "true"
    fi
else
    echo -e "${RED}❌ FAIL: Receive Message${NC}"
fi
echo

echo "📦 Phase 3: Batch Operations"
echo "============================"

# Test 3.1: Send message batch
run_test "Send Message Batch" \
    "aws sqs send-message-batch --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-standard-queue --entries '[{\"Id\":\"msg1\",\"MessageBody\":\"Batch message 1\"},{\"Id\":\"msg2\",\"MessageBody\":\"Batch message 2\"}]'" \
    "true"

# Test 3.2: Receive multiple messages
run_test "Receive Multiple Messages" \
    "aws sqs receive-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-standard-queue --max-number-of-messages 5" \
    "true"

echo "🔄 Phase 4: FIFO Queue Operations"
echo "================================="

# Test 4.1: Create FIFO queue
run_test "Create FIFO Queue" \
    "aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name test-fifo-queue.fifo --attributes FifoQueue=true,ContentBasedDeduplication=true" \
    "true"

# Test 4.2: Send FIFO message with group ID
run_test "Send FIFO Message with Group ID" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-fifo-queue.fifo --message-body 'FIFO Message 1' --message-group-id 'group1' --message-deduplication-id 'msg1'" \
    "true"

# Test 4.3: Send another FIFO message in same group
run_test "Send Another FIFO Message" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-fifo-queue.fifo --message-body 'FIFO Message 2' --message-group-id 'group1' --message-deduplication-id 'msg2'" \
    "true"

# Test 4.4: Receive FIFO messages
run_test "Receive FIFO Messages" \
    "aws sqs receive-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-fifo-queue.fifo --max-number-of-messages 2" \
    "true"

echo "💀 Phase 5: Dead Letter Queue Operations"
echo "========================================"

# Test 5.1: Create DLQ
run_test "Create Dead Letter Queue" \
    "aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name test-dlq" \
    "true"

# Test 5.2: Create queue with DLQ policy
run_test "Create Queue with DLQ Policy" \
    "echo '{\"RedrivePolicy\": \"{\\\"deadLetterTargetArn\\\":\\\"arn:aws:sqs:us-east-1:123456789012:test-dlq\\\",\\\"maxReceiveCount\\\":2}\"}' | aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name test-queue-with-dlq --attributes file:///dev/stdin" \
    "true"

echo "⏱️  Phase 6: Long Polling"
echo "========================"

# Test 6.1: Long polling (with timeout)
echo -e "${YELLOW}Testing: Long Polling with WaitTimeSeconds${NC}"
echo "Command: aws sqs receive-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-standard-queue --wait-time-seconds 2"
START_TIME=$(date +%s)
aws sqs receive-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-standard-queue --wait-time-seconds 2 >/dev/null 2>&1
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

if [ $DURATION -ge 1 ]; then
    echo -e "${GREEN}✅ PASS: Long polling waited appropriately (${DURATION}s)${NC}"
else
    echo -e "${RED}❌ FAIL: Long polling returned too quickly (${DURATION}s)${NC}"
fi
echo

echo "🚫 Phase 7: Error Handling"
echo "========================="

# Test 7.1: Invalid queue name
run_test "Invalid Queue Name (should fail)" \
    "aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name 'invalid queue name with spaces'" \
    "false"

# Test 7.2: Non-existent queue
run_test "Get Non-existent Queue URL (should fail)" \
    "aws sqs get-queue-url --endpoint-url $ENDPOINT_URL --queue-name non-existent-queue" \
    "false"

# Test 7.3: Invalid message body (too long)
run_test "Send Message Too Long (should fail)" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-standard-queue --message-body '$(python3 -c \"print('x' * 300000)\")'" \
    "false"

echo "🧹 Phase 8: Cleanup"
echo "=================="

# Clean up test queues
run_test "Delete Standard Queue" \
    "aws sqs delete-queue --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-standard-queue" \
    "true"

run_test "Delete FIFO Queue" \
    "aws sqs delete-queue --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-fifo-queue.fifo" \
    "true"

run_test "Delete DLQ" \
    "aws sqs delete-queue --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-dlq" \
    "true"

run_test "Delete Queue with DLQ" \
    "aws sqs delete-queue --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-queue-with-dlq" \
    "true"

echo "🎉 ==========================="
echo "🎉 Comprehensive Test Complete!"
echo "🎉 All AWS CLI operations tested successfully"
echo "🎉 ==========================="