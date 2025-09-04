#!/bin/bash

# Final validation test for QLite AWS CLI compatibility
# Focuses on core SQS operations that should work

set -e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}🧪 QLite AWS CLI Final Validation${NC}"
echo "=================================="

# Set up environment
export AWS_ACCESS_KEY_ID=dummy
export AWS_SECRET_ACCESS_KEY=dummy
export AWS_DEFAULT_REGION=us-east-1
ENDPOINT_URL="http://localhost:3000"

# Summary counters
PASSED=0
FAILED=0

# Function to run test
run_validation_test() {
    local name="$1"
    local command="$2"
    
    echo -e "\n${YELLOW}$name${NC}"
    echo "Command: $command"
    
    if eval "$command" >/dev/null 2>&1; then
        echo -e "${GREEN}✅ PASSED${NC}"
        ((PASSED++))
        return 0
    else
        echo -e "${RED}❌ FAILED${NC}"
        ((FAILED++))
        return 1
    fi
}

# Core queue operations
echo -e "${BLUE}Core Queue Operations${NC}"
run_validation_test "Create Queue" \
    "aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name validation-queue"

run_validation_test "List Queues" \
    "aws sqs list-queues --endpoint-url $ENDPOINT_URL"

run_validation_test "Get Queue URL" \
    "aws sqs get-queue-url --endpoint-url $ENDPOINT_URL --queue-name validation-queue"

# Message operations
echo -e "${BLUE}Message Operations${NC}"
run_validation_test "Send Message" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/validation-queue --message-body 'Hello World'"

run_validation_test "Receive Message" \
    "aws sqs receive-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/validation-queue"

# Queue attributes
echo -e "${BLUE}Queue Attributes${NC}"
run_validation_test "Set Queue Attributes" \
    "aws sqs set-queue-attributes --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/validation-queue --attributes VisibilityTimeout=30"

run_validation_test "Get Queue Attributes" \
    "aws sqs get-queue-attributes --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/validation-queue --attribute-names All"

# FIFO queue
echo -e "${BLUE}FIFO Queue Support${NC}"
run_validation_test "Create FIFO Queue" \
    "aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name validation-fifo.fifo --attributes FifoQueue=true"

run_validation_test "Send FIFO Message" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/validation-fifo.fifo --message-body 'FIFO message' --message-group-id group1 --message-deduplication-id msg1"

# Cleanup
echo -e "${BLUE}Cleanup${NC}"
run_validation_test "Delete Standard Queue" \
    "aws sqs delete-queue --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/validation-queue"

run_validation_test "Delete FIFO Queue" \
    "aws sqs delete-queue --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/validation-fifo.fifo"

# Summary
echo -e "\n${BLUE}📊 Test Summary${NC}"
echo "================"
echo -e "Passed: ${GREEN}$PASSED${NC}"
echo -e "Failed: ${RED}$FAILED${NC}"
TOTAL=$((PASSED + FAILED))
echo "Total: $TOTAL"

if [ $FAILED -eq 0 ]; then
    echo -e "\n${GREEN}🎉 All tests passed! QLite is working correctly with AWS CLI.${NC}"
else
    echo -e "\n${YELLOW}⚠️  Some tests failed. Review the failures above for specific issues.${NC}"
fi

# Success percentage
if [ $TOTAL -gt 0 ]; then
    PERCENTAGE=$(( (PASSED * 100) / TOTAL ))
    echo "Success rate: $PERCENTAGE%"
fi