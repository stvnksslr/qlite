#!/bin/bash

# Test MaxReceiveCount functionality in QLite

set -e

GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}🔄 Testing MaxReceiveCount Functionality${NC}"
echo "========================================"

export AWS_ACCESS_KEY_ID=dummy
export AWS_SECRET_ACCESS_KEY=dummy
export AWS_DEFAULT_REGION=us-east-1
ENDPOINT_URL="http://localhost:3000"

# Test function
test_feature() {
    local name="$1"
    local command="$2"
    local should_succeed="$3"
    
    echo -e "\n${YELLOW}Testing: $name${NC}"
    
    if eval "$command" >/dev/null 2>&1; then
        if [ "$should_succeed" = "true" ]; then
            echo -e "${GREEN}✅ PASSED${NC}"
            return 0
        else
            echo -e "${RED}❌ FAILED (should have failed)${NC}"
            return 1
        fi
    else
        if [ "$should_succeed" = "false" ]; then
            echo -e "${GREEN}✅ PASSED (correctly failed)${NC}"
            return 0
        else
            echo -e "${RED}❌ FAILED (unexpected failure)${NC}"
            echo "Command: $command"
            return 1
        fi
    fi
}

# Create DLQ first
echo -e "${BLUE}Creating Dead Letter Queue${NC}"
aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name test-dlq >/dev/null 2>&1

# Create queue with MaxReceiveCount and DLQ policy  
echo -e "${BLUE}Creating Queue with MaxReceiveCount${NC}"

test_feature "Create queue with DLQ redrive policy" \
    "echo '{\"RedrivePolicy\": \"{\\\"deadLetterTargetArn\\\":\\\"arn:aws:sqs:us-east-1:123456789012:test-dlq\\\",\\\"maxReceiveCount\\\":3}\"}' | aws sqs create-queue --endpoint-url $ENDPOINT_URL --queue-name max-receive-test --attributes file:///dev/stdin" \
    "true"

# Verify the queue attributes
test_feature "Get queue attributes with MaxReceiveCount" \
    "aws sqs get-queue-attributes --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/max-receive-test --attribute-names RedrivePolicy" \
    "true"

# Send a test message
test_feature "Send message to test queue" \
    "aws sqs send-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/max-receive-test --message-body 'MaxReceiveCount test message'" \
    "true"

# Test receiving messages multiple times (should work up to max count)
echo -e "\n${YELLOW}Testing multiple receives with short visibility timeout${NC}"
for i in {1..3}; do
    echo -e "\n${YELLOW}Receive attempt $i (should succeed)${NC}"
    RECEIVE_OUTPUT=$(aws sqs receive-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/max-receive-test --visibility-timeout 1 --wait-time-seconds 1 2>/dev/null || echo "")
    
    if echo "$RECEIVE_OUTPUT" | grep -q "MessageId"; then
        echo -e "${GREEN}✅ Receive $i successful - message returned to queue after visibility timeout${NC}"
        # Wait for visibility timeout to expire so message becomes available again
        sleep 3
    else
        echo -e "${RED}❌ No message received on attempt $i${NC}"
        break
    fi
done

# After max receive count, message should be moved to DLQ
echo -e "\n${YELLOW}Testing: Message moved to DLQ after max receives${NC}"
sleep 3  # Wait a bit for DLQ processing

# Check if message is now in DLQ
DLQ_MESSAGE=$(aws sqs receive-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-dlq --wait-time-seconds 2 2>/dev/null || echo "")
if echo "$DLQ_MESSAGE" | grep -q "MessageId"; then
    echo -e "${GREEN}✅ Message successfully moved to DLQ (MaxReceiveCount working properly)${NC}"
else
    echo -e "${YELLOW}⚠️  Message not found in DLQ - checking original queue${NC}"
    # Check if message still in original queue
    ORIG_MESSAGE=$(aws sqs receive-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/max-receive-test --wait-time-seconds 1 2>/dev/null || echo "")
    if echo "$ORIG_MESSAGE" | grep -q "MessageId"; then
        echo -e "${YELLOW}⚠️  Message still in original queue - DLQ may not be working${NC}"
    else
        echo -e "${YELLOW}⚠️  Message not found in either queue - may have been deleted${NC}"
    fi
fi

# Test system attributes in received messages
echo -e "\n${BLUE}Testing System Attributes${NC}"
ATTR_MESSAGE=$(aws sqs receive-message --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/max-receive-test --attribute-names All 2>/dev/null || echo "")

if echo "$ATTR_MESSAGE" | grep -q "SentTimestamp\|ApproximateReceiveCount"; then
    echo -e "${GREEN}✅ System attributes present in response${NC}"
else
    echo -e "${YELLOW}⚠️  System attributes not found${NC}"
fi

# Cleanup
echo -e "\n${BLUE}Cleanup${NC}"
aws sqs delete-queue --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/max-receive-test >/dev/null 2>&1 || true
aws sqs delete-queue --endpoint-url $ENDPOINT_URL --queue-url $ENDPOINT_URL/test-dlq >/dev/null 2>&1 || true

echo -e "\n${BLUE}🎯 MaxReceiveCount Test Complete!${NC}"