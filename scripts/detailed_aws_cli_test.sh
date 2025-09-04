#!/bin/bash

# Detailed AWS CLI test script for QLite
# Tests specific operations with detailed output

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🧪 Detailed QLite AWS CLI Test Suite${NC}"
echo "=================================================="

# Set credentials
export AWS_ACCESS_KEY_ID=dummy
export AWS_SECRET_ACCESS_KEY=dummy
export AWS_DEFAULT_REGION=us-east-1

ENDPOINT_URL="http://localhost:3000"

# Function to run test with detailed output
run_detailed_test() {
    local test_name="$1"
    local command="$2"
    
    echo -e "\n${YELLOW}Testing: $test_name${NC}"
    echo "Command: $command"
    echo "----------------------------------------"
    
    if output=$(eval "$command" 2>&1); then
        echo -e "${GREEN}✅ SUCCESS${NC}"
        echo "Output: $output"
    else
        echo -e "${RED}❌ FAILED${NC}"
        echo "Error: $output"
    fi
    echo
}

# Test 1: Basic operations
echo -e "${BLUE}Phase 1: Basic Queue Operations${NC}"
run_detailed_test "Create Standard Queue" \
    "aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name test-queue"

run_detailed_test "List Queues" \
    "aws sqs list-queues --endpoint-url $ENDPOINT_URL"

run_detailed_test "Get Queue URL" \
    "aws sqs get-queue-url --endpoint-url $ENDPOINT_URL --queue-name test-queue"

# Test 2: Message operations  
echo -e "${BLUE}Phase 2: Message Operations${NC}"
run_detailed_test "Send Message" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-queue --message-body 'Hello World'"

run_detailed_test "Send Message with Attributes" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-queue --message-body 'Message with attributes' --message-attributes 'Author={StringValue=QLite,DataType=String}'"

run_detailed_test "Receive Message" \
    "aws sqs receive-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-queue --max-number-of-messages 1"

# Test 3: Queue attributes
echo -e "${BLUE}Phase 3: Queue Attributes${NC}"
run_detailed_test "Set Queue Attributes" \
    "aws sqs set-queue-attributes --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-queue --attributes VisibilityTimeout=60,MessageRetentionPeriod=345600"

run_detailed_test "Get Queue Attributes" \
    "aws sqs get-queue-attributes --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-queue --attribute-names All"

# Test 4: FIFO queue
echo -e "${BLUE}Phase 4: FIFO Queue Operations${NC}"
run_detailed_test "Create FIFO Queue" \
    "aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name test-fifo.fifo --attributes FifoQueue=true"

run_detailed_test "Send FIFO Message" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-fifo.fifo --message-body 'FIFO message' --message-group-id 'group1' --message-deduplication-id 'msg1'"

# Test 5: Batch operations
echo -e "${BLUE}Phase 5: Batch Operations${NC}"
run_detailed_test "Send Message Batch (small)" \
    'aws sqs send-message-batch --endpoint-url '$ENDPOINT_URL' --queue-url '$ENDPOINT_URL'/test-queue --entries '"'"'[{"Id":"1","MessageBody":"Batch msg 1"},{"Id":"2","MessageBody":"Batch msg 2"}]'"'"

# Test 6: Error conditions
echo -e "${BLUE}Phase 6: Error Handling${NC}"
run_detailed_test "Invalid Queue Name (should fail)" \
    "aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name 'invalid queue name'"

run_detailed_test "Non-existent Queue (should fail)" \
    "aws sqs get-queue-url --endpoint-url $ENDPOINT_URL --queue-name non-existent-queue"

echo -e "${BLUE}🎉 Detailed Test Complete!${NC}"