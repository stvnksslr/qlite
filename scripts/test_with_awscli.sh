#!/bin/bash

echo "=== AWS CLI Integration Tests ==="
echo

# Set dummy AWS credentials
export AWS_ACCESS_KEY_ID=dummy
export AWS_SECRET_ACCESS_KEY=dummy

# Test 1: List queues
echo "1. Listing queues:"
timeout 10s aws sqs list-queues --endpoint-url http://localhost:3000 --debug 2>&1 | grep -E "(Response body:|ERROR|Exception)" | head -5
echo

# Test 2: Create queue
echo "2. Creating queue:"
timeout 10s aws sqs create-queue --endpoint-url http://localhost:3000 --queue-name aws-test-queue --debug 2>&1 | grep -E "(Response body:|ERROR|Exception)" | head -5
echo

# Test 3: Send message
echo "3. Sending message:"
timeout 10s aws sqs send-message --endpoint-url http://localhost:3000 --queue-url http://localhost:3000/aws-test-queue --message-body "Hello from AWS CLI" --debug 2>&1 | grep -E "(Response body:|ERROR|Exception)" | head -5
echo

# Test 4: Receive message
echo "4. Receiving message:"
timeout 10s aws sqs receive-message --endpoint-url http://localhost:3000 --queue-url http://localhost:3000/aws-test-queue --debug 2>&1 | grep -E "(Response body:|ERROR|Exception)" | head -5
echo

echo "=== Integration Test Complete ==="