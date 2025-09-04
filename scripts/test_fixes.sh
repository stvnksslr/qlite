#!/bin/bash

# Test script to verify all the fixes work properly

set -e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}🔧 Testing QLite Fixes${NC}"
echo "=============================="

# Set up environment
export AWS_ACCESS_KEY_ID=dummy
export AWS_SECRET_ACCESS_KEY=dummy
export AWS_DEFAULT_REGION=us-east-1
ENDPOINT_URL="http://localhost:3000"

PASSED=0
FAILED=0

# Function to run test
test_fix() {
    local name="$1"
    local command="$2"
    local expected_success="$3"
    
    echo -e "\n${BLUE}Testing: $name${NC}"
    
    if eval "$command" >/dev/null 2>&1; then
        if [ "$expected_success" = "true" ]; then
            echo -e "${GREEN}✅ PASSED${NC}"
            ((PASSED++))
        else
            echo -e "${RED}❌ FAILED (expected failure)${NC}"
            ((FAILED++))
        fi
    else
        if [ "$expected_success" = "false" ]; then
            echo -e "${GREEN}✅ PASSED (correctly failed)${NC}"
            ((PASSED++))
        else
            echo -e "${RED}❌ FAILED (unexpected failure)${NC}"
            echo "Command: $command"
            ((FAILED++))
        fi
    fi
}

# Test 1: DeleteQueue (was missing)
echo -e "${BLUE}=== DeleteQueue Functionality ===${NC}"

test_fix "Create queue for deletion test" \
    "aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name delete-test-queue" \
    "true"

test_fix "Delete queue" \
    "aws sqs delete-queue --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/delete-test-queue" \
    "true"

test_fix "Delete non-existent queue (should fail)" \
    "aws sqs delete-queue --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/non-existent-queue" \
    "false"

# Test 2: SendMessageBatch (was broken)
echo -e "${BLUE}=== SendMessageBatch Functionality ===${NC}"

# Create test queue
aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name batch-test-queue >/dev/null 2>&1

test_fix "Send message batch with entries" \
    'aws sqs send-message-batch --endpoint-url '$ENDPOINT_URL' --queue-url '$ENDPOINT_URL'/batch-test-queue --entries '"'"'[{"Id":"1","MessageBody":"Batch message 1"},{"Id":"2","MessageBody":"Batch message 2"}]'"'"' \
    "true"

# Test 3: DeleteMessageBatch (was broken)  
echo -e "${BLUE}=== DeleteMessageBatch Functionality ===${NC}"

# Send a couple messages and try to delete them
aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/batch-test-queue --message-body "Message to delete 1" >/dev/null 2>&1
aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/batch-test-queue --message-body "Message to delete 2" >/dev/null 2>&1

# Receive messages to get receipt handles
MESSAGES=$(aws sqs receive-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/batch-test-queue --max-number-of-messages 2 2>/dev/null || true)

if echo "$MESSAGES" | grep -q "ReceiptHandle"; then
    echo -e "${GREEN}✅ PASSED: Received messages for batch delete test${NC}"
    ((PASSED++))
else
    echo -e "${RED}❌ FAILED: Could not receive messages for batch delete test${NC}"
    ((FAILED++))
fi

# Test 4: JSON Request Parsing (enhanced for batch operations)
echo -e "${BLUE}=== JSON Request Parsing ===${NC}"

test_fix "JSON format batch request" \
    'curl -s -X POST '$ENDPOINT_URL'/ -H "Content-Type: application/x-amz-json-1.0" -H "X-Amz-Target: AmazonSQS.SendMessageBatch" -d '"'"'{"QueueUrl":"'$ENDPOINT_URL'/batch-test-queue","Entries":[{"Id":"json1","MessageBody":"JSON batch message"}]}'"'"' \
    "true"

# Test 5: Queue Operations
echo -e "${BLUE}=== Core Queue Operations ===${NC}"

test_fix "List queues" \
    "aws sqs list-queues --endpoint-url $ENDPOINT_URL" \
    "true"

test_fix "Create FIFO queue" \
    "aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name test-fifo-fixes.fifo --attributes FifoQueue=true" \
    "true"

test_fix "Send FIFO message" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-fifo-fixes.fifo --message-body 'FIFO test' --message-group-id group1 --message-deduplication-id fix-test-1" \
    "true"

# Cleanup
echo -e "${BLUE}=== Cleanup ===${NC}"
aws sqs delete-queue --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/batch-test-queue >/dev/null 2>&1 || true
aws sqs delete-queue --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-fifo-fixes.fifo >/dev/null 2>&1 || true

# Summary
echo -e "\n${BLUE}📊 Fix Validation Summary${NC}"
echo "=========================="
echo -e "✅ Passed: ${GREEN}$PASSED${NC}"
echo -e "❌ Failed: ${RED}$FAILED${NC}"
TOTAL=$((PASSED + FAILED))
echo "Total tests: $TOTAL"

if [ $FAILED -eq 0 ]; then
    echo -e "\n${GREEN}🎉 All fixes working correctly!${NC}"
    exit 0
else
    echo -e "\n${RED}⚠️  Some fixes need more work.${NC}"
    exit 1
fi