//! Test Fixtures and Constants
//!
//! Provides test data, constants, and sample configurations for tests.

/// Default test username
pub const TEST_USERNAME: &str = "testuser";

/// Default test password
pub const TEST_PASSWORD: &str = "testpass";

/// Alternative test username
pub const ALT_USERNAME: &str = "altuser";

/// Alternative test password
pub const ALT_PASSWORD: &str = "altpass";

/// Invalid password for negative tests
pub const INVALID_PASSWORD: &str = "wrongpass";

/// Default terminal columns
pub const DEFAULT_COLS: u16 = 80;

/// Default terminal rows
pub const DEFAULT_ROWS: u16 = 24;

/// Large terminal columns
pub const LARGE_COLS: u16 = 160;

/// Large terminal rows
pub const LARGE_ROWS: u16 = 50;

/// Sample commands for testing
pub mod commands {
    /// Simple echo command
    pub const ECHO_HELLO: &str = "echo hello";
    /// Expected output for echo hello
    pub const ECHO_HELLO_OUTPUT: &str = "hello";

    /// List directory command
    pub const LS: &str = "ls";

    /// Print working directory
    pub const PWD: &str = "pwd";

    /// Whoami command
    pub const WHOAMI: &str = "whoami";

    /// Exit command
    pub const EXIT: &str = "exit";

    /// Command that should fail
    pub const INVALID_CMD: &str = "nonexistent_command_12345";
}

/// Sample scripted responses for mock server
pub fn sample_scripted_responses() -> Vec<(String, String)> {
    vec![
        ("echo hello".to_string(), "hello".to_string()),
        ("whoami".to_string(), TEST_USERNAME.to_string()),
        ("pwd".to_string(), "/home/testuser".to_string()),
        ("ls".to_string(), "file1.txt\nfile2.txt\ndir1".to_string()),
        (
            "uname -a".to_string(),
            "Linux mockhost 5.10.0-mock #1 SMP x86_64 GNU/Linux".to_string(),
        ),
    ]
}

/// Connection timeout for tests (milliseconds)
pub const CONNECTION_TIMEOUT_MS: u64 = 5000;

/// Short timeout for testing timeout behavior
pub const SHORT_TIMEOUT_MS: u64 = 100;

/// Wait time for server startup (milliseconds)
pub const SERVER_STARTUP_WAIT_MS: u64 = 100;
