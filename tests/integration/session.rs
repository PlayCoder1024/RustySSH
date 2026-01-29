//! Session Integration Tests
//!
//! Tests for terminal session management including PTY handling and resize.

use crate::common::*;
use rustyssh::ssh::{Session, SessionManager};
use uuid::Uuid;

/// Test creating a new session
#[test]
fn test_session_creation() {
    let host_id = Uuid::new_v4();
    let session = Session::new(
        host_id,
        "test-session".to_string(),
        DEFAULT_COLS,
        DEFAULT_ROWS,
    );

    assert_eq!(session.host_id, host_id);
    assert_eq!(session.name, "test-session");
    assert_eq!(session.cols, DEFAULT_COLS);
    assert_eq!(session.rows, DEFAULT_ROWS);
    assert_eq!(session.scroll_offset, 0);
}

/// Test session data processing through VT100 parser
#[test]
fn test_session_data_processing() {
    let host_id = Uuid::new_v4();
    let mut session = Session::new(host_id, "test".to_string(), 80, 24);

    // Process some terminal data
    session.process_data(b"Hello, World!\r\n");

    // Get screen content
    let lines = session.screen_lines();
    assert!(!lines.is_empty());

    // First line should contain our text
    assert!(
        lines[0].contains("Hello, World!"),
        "Screen should contain processed text, got: {:?}",
        lines
    );
}

/// Test session resize
#[test]
fn test_session_resize() {
    let host_id = Uuid::new_v4();
    let mut session = Session::new(host_id, "test".to_string(), DEFAULT_COLS, DEFAULT_ROWS);

    assert_eq!(session.cols, DEFAULT_COLS);
    assert_eq!(session.rows, DEFAULT_ROWS);

    // Resize to larger dimensions
    session.resize(LARGE_COLS, LARGE_ROWS);

    assert_eq!(session.cols, LARGE_COLS);
    assert_eq!(session.rows, LARGE_ROWS);
}

/// Test session scrolling
#[test]
fn test_session_scrolling() {
    let host_id = Uuid::new_v4();
    let mut session = Session::new(host_id, "test".to_string(), 80, 24);

    // Fill screen with data
    for i in 0..50 {
        session.process_data(format!("Line {}\r\n", i).as_bytes());
    }

    // Should be at bottom
    assert_eq!(session.scroll_offset, 0);

    // Scroll up
    session.scroll_up(10);
    assert!(session.scroll_offset > 0);

    // Scroll down
    let prev_offset = session.scroll_offset;
    session.scroll_down(5);
    assert!(session.scroll_offset < prev_offset);

    // Scroll to bottom
    session.scroll_to_bottom();
    assert_eq!(session.scroll_offset, 0);
}

/// Test cursor position and visibility
#[test]
fn test_session_cursor() {
    let host_id = Uuid::new_v4();
    let session = Session::new(host_id, "test".to_string(), 80, 24);

    let (row, col) = session.cursor_position();
    assert!(row < 24);
    assert!(col < 80);

    // Cursor should be visible by default
    assert!(session.cursor_visible());
}

/// Test SessionManager creation and basic operations
#[test]
fn test_session_manager_basic() {
    let mut manager = SessionManager::new();
    let host_id = Uuid::new_v4();

    // Create a session
    let session_id = manager.create_session(host_id, "test-session".to_string(), 80, 24);

    // Retrieve the session
    let session = manager.get(session_id);
    assert!(session.is_some());
    assert_eq!(session.unwrap().name, "test-session");

    // Get mutable reference
    let session_mut = manager.get_mut(session_id);
    assert!(session_mut.is_some());

    // List sessions
    let sessions = manager.list();
    assert_eq!(sessions.len(), 1);

    // Remove session
    let removed = manager.remove(session_id);
    assert!(removed.is_some());

    // Session should no longer exist
    assert!(manager.get(session_id).is_none());
    assert!(manager.list().is_empty());
}

/// Test SessionManager with multiple sessions
#[test]
fn test_session_manager_multiple() {
    let mut manager = SessionManager::new();

    // Create multiple sessions
    let id1 = manager.create_session(Uuid::new_v4(), "session1".to_string(), 80, 24);
    let id2 = manager.create_session(Uuid::new_v4(), "session2".to_string(), 100, 30);
    let id3 = manager.create_session(Uuid::new_v4(), "session3".to_string(), 120, 40);

    // All should exist
    assert!(manager.get(id1).is_some());
    assert!(manager.get(id2).is_some());
    assert!(manager.get(id3).is_some());
    assert_eq!(manager.list().len(), 3);

    // Remove one
    manager.remove(id2);
    assert!(manager.get(id2).is_none());
    assert_eq!(manager.list().len(), 2);

    // Others should still exist
    assert!(manager.get(id1).is_some());
    assert!(manager.get(id3).is_some());
}

/// Test SessionManager process data
#[tokio::test]
async fn test_session_manager_process_data() {
    let mut manager = SessionManager::new();
    let host_id = Uuid::new_v4();

    let session_id = manager.create_session(host_id, "test".to_string(), 80, 24);

    // Process data through manager
    manager
        .process_data(session_id, b"Test data\r\n")
        .await
        .expect("Should process data");

    // Verify data was processed
    let session = manager.get(session_id).expect("Session should exist");
    let lines = session.screen_lines();
    assert!(
        lines.iter().any(|l| l.contains("Test data")),
        "Session should contain processed data"
    );
}

/// Test SessionManager resize
#[tokio::test]
async fn test_session_manager_resize() {
    let mut manager = SessionManager::new();
    let host_id = Uuid::new_v4();

    let session_id = manager.create_session(host_id, "test".to_string(), 80, 24);

    // Resize through manager
    manager
        .resize_session(session_id, 120, 40)
        .await
        .expect("Should resize");

    let session = manager.get(session_id).expect("Session should exist");
    assert_eq!(session.cols, 120);
    assert_eq!(session.rows, 40);
}
