#!/bin/bash

# Issue-focused AWS CLI test for QLite
# Tests specific problems and validates expected behaviors

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}🔍 Issue-Focused QLite Test Suite${NC}"
echo "=============================================="

# Set credentials
export AWS_ACCESS_KEY_ID=dummy
export AWS_SECRET_ACCESS_KEY=dummy
export AWS_DEFAULT_REGION=us-east-1
ENDPOINT_URL="http://localhost:3000"

# Function to test with expected output
test_with_expected_output() {
    local test_name="$1"
    local command="$2"
    local should_succeed="$3"
    
    echo -e "\n${YELLOW}Testing: $test_name${NC}"
    echo "Command: $command"
    echo "Expected: $([ "$should_succeed" == "true" ] && echo "SUCCESS" || echo "FAILURE")"
    echo "----------------------------------------"
    
    if output=$(eval "$command" 2>&1); then
        if [ "$should_succeed" == "true" ]; then
            echo -e "${GREEN}✅ PASS - Command succeeded as expected${NC}"
            if [ -n "$output" ]; then
                echo "Output: $output"
            fi
        else
            echo -e "${RED}❌ FAIL - Command succeeded but should have failed${NC}"
            echo "Output: $output"
        fi
    else
        if [ "$should_succeed" == "false" ]; then
            echo -e "${GREEN}✅ PASS - Command failed as expected${NC}"
            echo "Error: $output"
        else
            echo -e "${RED}❌ FAIL - Command failed unexpectedly${NC}"
            echo "Error: $output"
        fi
    fi
}

# Issue 1: Output formatting - AWS CLI should show proper JSON responses
echo -e "${BLUE}Issue 1: Response Format Testing${NC}"

test_with_expected_output "Create Queue with JSON Output" \
    "aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name output-test --output json" \
    "true"

test_with_expected_output "List Queues with JSON Output" \
    "aws sqs list-queues --endpoint-url $ENDPOINT_URL --output json" \
    "true"

# Issue 2: Batch operations 
echo -e "${BLUE}Issue 2: Batch Operations${NC}"

# Create a queue for batch testing
aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name batch-test > /dev/null 2>&1

test_with_expected_output "Send Message Batch with Simple Entries" \
    'aws sqs send-message-batch --endpoint-url '$ENDPOINT_URL' --queue-url '$ENDPOINT_URL'/batch-test --entries Id=1,MessageBody="Msg1" Id=2,MessageBody="Msg2"' \
    "true"

test_with_expected_output "Send Message Batch with JSON Entries" \
    'aws sqs send-message-batch --endpoint-url '$ENDPOINT_URL' --queue-url '$ENDPOINT_URL'/batch-test --entries '"'"'[{"Id":"msg1","MessageBody":"Test message 1"},{"Id":"msg2","MessageBody":"Test message 2"}]'"'"' \
    "true"

# Issue 3: Message content and attributes
echo -e "${BLUE}Issue 3: Message Content and Attributes${NC}"

# Create a queue for message testing  
aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name message-test > /dev/null 2>&1

test_with_expected_output "Send Message with Complex Body" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/message-test --message-body 'Complex message with special chars'" \
    "true"

test_with_expected_output "Send Message with Multiple Attributes" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/message-test --message-body 'Message with attributes' --message-attributes 'Author={StringValue=Test,DataType=String}'" \
    "true"

# Issue 4: Error validation
echo -e "${BLUE}Issue 4: Error Validation${NC}"

test_with_expected_output "Invalid Queue Name with Spaces (should fail)" \
    "aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name 'invalid queue name'" \
    "false"

test_with_expected_output "Queue Name Too Long (should fail)" \
    "aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name '$(python3 -c \"print('x' * 81)\")'" \
    "false"

test_with_expected_output "Empty Message Body (should fail)" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/message-test --message-body ''" \
    "false"

# Issue 5: Receive message output format
echo -e "${BLUE}Issue 5: Message Receive Testing${NC}"

# Send a message first
aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/message-test --message-body "Test receive message" > /dev/null 2>&1

test_with_expected_output "Receive Message with JSON Output" \
    "aws sqs receive-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/message-test --output json" \
    "true"

test_with_expected_output "Receive Message with All Attributes" \
    "aws sqs receive-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/message-test --attribute-names All --message-attribute-names All --output json" \
    "true"

# Issue 6: Long polling
echo -e "${BLUE}Issue 6: Long Polling${NC}"

echo -e "${YELLOW}Testing: Long Polling (2 second wait)${NC}"
START_TIME=$(date +%s)
test_with_expected_output "Long Polling Test" \
    "aws sqs receive-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/message-test --wait-time-seconds 2" \
    "true"
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))
echo "Wait duration: ${DURATION}s"

echo -e "${BLUE}🎯 Issue-Focused Test Complete!${NC}"
echo "Key findings will help identify specific areas needing fixes."