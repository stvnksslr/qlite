use tempfile::TempDir;

use qlite::database::Database;
use qlite::queue_service::QueueService;

/// Simple integration test to verify the testing infrastructure works
#[tokio::test]
async fn test_basic_queue_operations() {
    // Setup test environment
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let db_path = temp_dir.path().join("test.db");
    let service = QueueService::new(db_path.to_str().unwrap())
        .await
        .expect("Failed to create queue service");

    // Test creating a queue
    service
        .create_queue("test-queue")
        .await
        .expect("Failed to create queue");

    // Test listing queues
    let queues = service.list_queues().await.expect("Failed to list queues");
    assert_eq!(queues.len(), 1);
    assert_eq!(queues[0].0, "test-queue");

    // Test sending a message
    let message_id = service
        .send_message("test-queue", "Hello World", None, None)
        .await
        .expect("Failed to send message");
    assert!(!message_id.is_empty());

    // Test receiving a message
    let received = service
        .receive_message("test-queue")
        .await
        .expect("Failed to receive message");
    assert!(received.is_some());
    let msg = received.unwrap();
    assert_eq!(msg.body, "Hello World");

    // Test getting queue attributes
    let attrs = service
        .get_queue_attributes("test-queue")
        .await
        .expect("Failed to get queue attributes");
    assert!(attrs.is_some());
}

#[tokio::test]
async fn test_database_operations() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let db_path = temp_dir.path().join("test_db.db");
    let db = Database::new(db_path.to_str().unwrap())
        .await
        .expect("Failed to create database");

    // Test queue creation
    db.create_queue("db-test-queue")
        .await
        .expect("Failed to create queue in database");

    // Test sending message directly to database
    db.send_message(
        "db-test-queue",
        "msg-id-1",
        "Database test message",
        None,
        None,
    )
    .await
    .expect("Failed to send message to database");

    // Test listing queues
    let queues = db
        .list_queues()
        .await
        .expect("Failed to list queues from database");
    assert_eq!(queues.len(), 1);
    assert_eq!(queues[0].0, "db-test-queue");
}

#[tokio::test]
async fn test_message_lifecycle() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let db_path = temp_dir.path().join("lifecycle_test.db");
    let service = QueueService::new(db_path.to_str().unwrap())
        .await
        .expect("Failed to create queue service");

    // Create queue
    service
        .create_queue("lifecycle-queue")
        .await
        .expect("Failed to create queue");

    // Send message
    let message_id = service
        .send_message("lifecycle-queue", "Lifecycle test", None, None)
        .await
        .expect("Failed to send message");

    // Receive message
    let received = service
        .receive_message("lifecycle-queue")
        .await
        .expect("Failed to receive message");
    assert!(received.is_some());
    let msg = received.unwrap();
    assert_eq!(msg.body, "Lifecycle test");
    let receipt_handle = msg.receipt_handle;

    // Delete message
    let deleted = service
        .delete_message(&receipt_handle)
        .await
        .expect("Failed to delete message");
    assert!(deleted);

    // Verify message is deleted by trying to receive again
    let received_after_delete = service
        .receive_message("lifecycle-queue")
        .await
        .expect("Failed to attempt receive after delete");
    assert!(received_after_delete.is_none());

    // Restore message
    let restored = service
        .restore_message(&message_id)
        .await
        .expect("Failed to restore message");
    assert!(restored);
}

#[tokio::test]
async fn test_queue_attributes() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let db_path = temp_dir.path().join("attrs_test.db");
    let service = QueueService::new(db_path.to_str().unwrap())
        .await
        .expect("Failed to create queue service");

    // Create queue
    service
        .create_queue("attrs-queue")
        .await
        .expect("Failed to create queue");

    // Get initial attributes
    let attrs = service
        .get_queue_attributes("attrs-queue")
        .await
        .expect("Failed to get queue attributes");
    assert!(attrs.is_some());
    let queue_attrs = attrs.unwrap();
    assert_eq!(queue_attrs.approximate_number_of_messages, 0);

    // Send some messages
    for i in 1..=3 {
        service
            .send_message("attrs-queue", &format!("Message {}", i), None, None)
            .await
            .expect("Failed to send message");
    }

    // Check updated attributes
    let updated_attrs = service
        .get_queue_attributes("attrs-queue")
        .await
        .expect("Failed to get updated queue attributes");
    assert!(updated_attrs.is_some());
    let queue_attrs = updated_attrs.unwrap();
    assert_eq!(queue_attrs.approximate_number_of_messages, 3);
}

#[tokio::test]
async fn test_multiple_queues() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let db_path = temp_dir.path().join("multi_queue_test.db");
    let service = QueueService::new(db_path.to_str().unwrap())
        .await
        .expect("Failed to create queue service");

    // Create multiple queues
    let queue_names = vec!["queue-1", "queue-2", "queue-3"];
    for queue_name in &queue_names {
        service
            .create_queue(queue_name)
            .await
            .expect("Failed to create queue");
    }

    // Send messages to each queue
    for queue_name in queue_names.iter() {
        service
            .send_message(
                queue_name,
                &format!("Message for {}", queue_name),
                None,
                None,
            )
            .await
            .expect("Failed to send message");
    }

    // Verify each queue has its own message
    for queue_name in &queue_names {
        let received = service
            .receive_message(queue_name)
            .await
            .expect("Failed to receive message");
        assert!(received.is_some());
        let msg = received.unwrap();
        assert!(msg.body.contains(queue_name));
    }

    // Verify queue list
    let queues = service.list_queues().await.expect("Failed to list queues");
    assert_eq!(queues.len(), 3);

    let queue_names_from_list: Vec<&str> = queues.iter().map(|(name, _)| name.as_str()).collect();
    for expected_name in &queue_names {
        assert!(queue_names_from_list.contains(expected_name));
    }
}
