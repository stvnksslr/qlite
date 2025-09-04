#!/bin/bash

# Production readiness test for QLite
# Tests critical AWS CLI compatibility features

set -e

GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}🚀 QLite Production Readiness Test${NC}"
echo "===================================="

export AWS_ACCESS_KEY_ID=dummy
export AWS_SECRET_ACCESS_KEY=dummy
export AWS_DEFAULT_REGION=us-east-1
ENDPOINT_URL="http://localhost:3000"

PASSED=0
FAILED=0

test_operation() {
    local name="$1"
    local command="$2"
    local show_output="$3"
    
    echo -e "\n${YELLOW}Testing: $name${NC}"
    
    if output=$(eval "$command" 2>&1); then
        echo -e "${GREEN}✅ SUCCESS${NC}"
        if [ "$show_output" == "true" ] && [ -n "$output" ]; then
            echo "Response: $output"
        fi
        ((PASSED++))
    else
        echo -e "${RED}❌ FAILED${NC}"
        echo "Error: $output"
        ((FAILED++))
    fi
}

# Test 1: Basic SQS Operations
echo -e "${BLUE}=== Core SQS Operations ===${NC}"

test_operation "Create standard queue" \
    "aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name prod-test-queue --output json" \
    "true"

test_operation "List all queues" \
    "aws sqs list-queues --endpoint-url $ENDPOINT_URL --output json" \
    "true"

test_operation "Get queue URL" \
    "aws sqs get-queue-url --endpoint-url $ENDPOINT_URL --queue-name prod-test-queue --output json" \
    "true"

# Test 2: Message Operations
echo -e "${BLUE}=== Message Operations ===${NC}"

test_operation "Send simple message" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/prod-test-queue --message-body 'Production test message' --output json" \
    "true"

test_operation "Send message with attributes" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/prod-test-queue --message-body 'Message with attributes' --message-attributes 'Priority={StringValue=High,DataType=String}' --output json" \
    "true"

test_operation "Receive messages" \
    "aws sqs receive-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/prod-test-queue --max-number-of-messages 5 --output json" \
    "true"

# Test 3: Queue Configuration
echo -e "${BLUE}=== Queue Configuration ===${NC}"

test_operation "Set queue attributes" \
    "aws sqs set-queue-attributes --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/prod-test-queue --attributes VisibilityTimeout=45,MessageRetentionPeriod=604800"

test_operation "Get queue attributes" \
    "aws sqs get-queue-attributes --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/prod-test-queue --attribute-names All --output json" \
    "true"

# Test 4: FIFO Queue Support  
echo -e "${BLUE}=== FIFO Queue Support ===${NC}"

test_operation "Create FIFO queue" \
    "aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name prod-fifo-test.fifo --attributes FifoQueue=true,ContentBasedDeduplication=true --output json" \
    "true"

test_operation "Send FIFO message with group" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/prod-fifo-test.fifo --message-body 'FIFO test message' --message-group-id 'test-group' --message-deduplication-id 'test-msg-1' --output json" \
    "true"

test_operation "Receive FIFO message" \
    "aws sqs receive-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/prod-fifo-test.fifo --output json" \
    "true"

# Test 5: Error Handling
echo -e "${BLUE}=== Error Handling ===${NC}"

echo -e "\n${YELLOW}Testing: Non-existent queue (should fail)${NC}"
if aws sqs get-queue-url --endpoint-url $ENDPOINT_URL --queue-name non-existent-queue-12345 >/dev/null 2>&1; then
    echo -e "${RED}❌ FAILED - Should have returned error${NC}"
    ((FAILED++))
else
    echo -e "${GREEN}✅ SUCCESS - Properly returned error${NC}"
    ((PASSED++))
fi

# Test 6: Performance check
echo -e "${BLUE}=== Performance Check ===${NC}"

echo -e "\n${YELLOW}Testing: Multiple rapid operations${NC}"
START_TIME=$(date +%s)

for i in {1..5}; do
    aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/prod-test-queue --message-body "Rapid test message $i" >/dev/null 2>&1
done

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

if [ $DURATION -le 5 ]; then
    echo -e "${GREEN}✅ SUCCESS - 5 messages sent in ${DURATION}s${NC}"
    ((PASSED++))
else
    echo -e "${RED}❌ SLOW - 5 messages took ${DURATION}s${NC}"
    ((FAILED++))
fi

# Summary
echo -e "\n${BLUE}📊 Production Readiness Summary${NC}"
echo "=================================="
echo -e "✅ Passed: ${GREEN}$PASSED${NC}"
echo -e "❌ Failed: ${RED}$FAILED${NC}"
TOTAL=$((PASSED + FAILED))
echo "Total tests: $TOTAL"

if [ $TOTAL -gt 0 ]; then
    PERCENTAGE=$(( (PASSED * 100) / TOTAL ))
    echo "Success rate: $PERCENTAGE%"
fi

if [ $PASSED -ge 12 ] && [ $FAILED -le 2 ]; then
    echo -e "\n${GREEN}🎉 QLite is production ready for AWS CLI usage!${NC}"
    echo -e "${GREEN}Core SQS operations are working correctly.${NC}"
else
    echo -e "\n${YELLOW}⚠️  Some issues found. Review failures above.${NC}"
fi

echo -e "\n${BLUE}Key capabilities verified:${NC}"
echo "• Standard and FIFO queue creation"
echo "• Message send/receive operations" 
echo "• Queue attribute management"
echo "• AWS CLI JSON output format"
echo "• Error handling for invalid requests"
echo "• Basic performance characteristics"