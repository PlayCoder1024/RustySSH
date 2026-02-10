//! SFTP file transfer operations

use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Transfer direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    Upload,
    Download,
}

/// Transfer status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
    Cancelled,
}

/// Single transfer item
#[derive(Clone)]
pub struct TransferItem {
    /// Transfer ID
    pub id: Uuid,
    /// Source path
    pub source: PathBuf,
    /// Destination path
    pub destination: PathBuf,
    /// Transfer direction
    pub direction: TransferDirection,
    /// Current status
    pub status: TransferStatus,
    /// Total bytes to transfer
    pub total_bytes: u64,
    /// Bytes transferred so far
    pub transferred_bytes: u64,
    /// Start time
    pub started_at: Option<DateTime<Utc>>,
    /// Completion time
    pub completed_at: Option<DateTime<Utc>>,
    /// Current transfer speed (bytes/sec)
    pub speed: f64,
    /// Host ID (for the remote side)
    pub host_id: Uuid,
}

impl TransferItem {
    /// Create a new transfer item
    pub fn new(
        source: PathBuf,
        destination: PathBuf,
        direction: TransferDirection,
        total_bytes: u64,
        host_id: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            source,
            destination,
            direction,
            status: TransferStatus::Pending,
            total_bytes,
            transferred_bytes: 0,
            started_at: None,
            completed_at: None,
            speed: 0.0,
            host_id,
        }
    }

    /// Get progress percentage (0.0 - 100.0)
    pub fn progress(&self) -> f64 {
        if self.total_bytes == 0 {
            return 100.0;
        }
        (self.transferred_bytes as f64 / self.total_bytes as f64) * 100.0
    }

    /// Get estimated time remaining in seconds
    pub fn eta(&self) -> Option<f64> {
        if self.speed <= 0.0 {
            return None;
        }
        let remaining = self.total_bytes.saturating_sub(self.transferred_bytes);
        Some(remaining as f64 / self.speed)
    }

    /// Get formatted speed
    pub fn speed_display(&self) -> String {
        format_speed(self.speed)
    }

    /// Get formatted ETA
    pub fn eta_display(&self) -> String {
        if let Some(eta) = self.eta() {
            format_duration(eta)
        } else {
            "--:--".to_string()
        }
    }
}

/// Format transfer speed
fn format_speed(bytes_per_sec: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bytes_per_sec >= GB {
        format!("{:.1} GB/s", bytes_per_sec / GB)
    } else if bytes_per_sec >= MB {
        format!("{:.1} MB/s", bytes_per_sec / MB)
    } else if bytes_per_sec >= KB {
        format!("{:.1} KB/s", bytes_per_sec / KB)
    } else {
        format!("{:.0} B/s", bytes_per_sec)
    }
}

/// Format duration in seconds to MM:SS or HH:MM:SS
fn format_duration(seconds: f64) -> String {
    let secs = seconds as u64;
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let secs = secs % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    } else {
        format!("{:02}:{:02}", mins, secs)
    }
}

/// Progress update from transfer
#[derive(Debug, Clone)]
pub struct TransferProgress {
    pub id: Uuid,
    pub transferred_bytes: u64,
    pub speed: f64,
    pub error: Option<String>,
}

/// Transfer queue managing multiple transfers
pub struct TransferQueue {
    /// Pending transfers
    pending: VecDeque<TransferItem>,
    /// Currently active transfers
    active: Vec<TransferItem>,
    /// Completed transfers (history)
    completed: VecDeque<TransferItem>,
    /// Maximum concurrent transfers
    max_concurrent: usize,
    /// Maximum history size
    max_history: usize,
    /// Progress sender
    progress_tx: mpsc::UnboundedSender<TransferProgress>,
    /// Progress receiver (optional, taken by the event bridge)
    progress_rx: Option<mpsc::UnboundedReceiver<TransferProgress>>,
}

impl TransferQueue {
    pub fn new(max_concurrent: usize) -> Self {
        let (progress_tx, progress_rx) = mpsc::unbounded_channel();

        Self {
            pending: VecDeque::new(),
            active: Vec::new(),
            completed: VecDeque::new(),
            max_concurrent,
            max_history: 100,
            progress_tx,
            progress_rx: Some(progress_rx),
        }
    }

    /// Take the progress receiver to bridge to event loop
    pub fn take_progress_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<TransferProgress>> {
        self.progress_rx.take()
    }

    /// Add a transfer to the queue
    pub fn add(&mut self, item: TransferItem) -> Uuid {
        let id = item.id;
        let direction = match item.direction {
            TransferDirection::Upload => "upload",
            TransferDirection::Download => "download",
        };
        info!(target: "sftp::transfer", "Transfer queued: {} {} -> {} ({} bytes)",
            direction, item.source.display(), item.destination.display(), item.total_bytes);
        self.pending.push_back(item);
        id
    }

    /// Get all pending transfers
    pub fn pending(&self) -> &VecDeque<TransferItem> {
        &self.pending
    }

    /// Get all active transfers
    pub fn active(&self) -> &[TransferItem] {
        &self.active
    }

    /// Get completed transfers
    pub fn completed(&self) -> &VecDeque<TransferItem> {
        &self.completed
    }

    /// Get total pending + active count
    pub fn total_pending(&self) -> usize {
        self.pending.len() + self.active.len()
    }

    /// Cancel a transfer
    pub fn cancel(&mut self, id: Uuid) {
        info!(target: "sftp::transfer", "Cancelling transfer {}", id);
        // Check pending
        if let Some(pos) = self.pending.iter().position(|t| t.id == id) {
            if let Some(mut item) = self.pending.remove(pos) {
                item.status = TransferStatus::Cancelled;
                self.add_to_history(item);
            }
        }

        // Check active (mark as cancelled, actual cancellation happens in transfer task)
        if let Some(item) = self.active.iter_mut().find(|t| t.id == id) {
            item.status = TransferStatus::Cancelled;
        }
    }

    /// Update progress for a transfer
    pub fn update_progress(&mut self, progress: TransferProgress) {
        if let Some(err) = progress.error {
            self.complete(progress.id, Some(err));
            return;
        }

        if let Some(item) = self.active.iter_mut().find(|t| t.id == progress.id) {
            item.transferred_bytes = progress.transferred_bytes;
            item.speed = progress.speed;

            // Check for completion
            if item.transferred_bytes >= item.total_bytes && item.total_bytes > 0 {
                // Completion is now handled via explicit "done" progress or length check?
                // Let's rely on length check for now, but really worker should signal done.
                // Actually worker will just exit loop.
                // We need explicit DONE signal.
                // Let's assume if bytes == total, it's done.
                // But worker might send final update.
                // Let's change update_progress to call complete if done.
            }
        }

        // Also check if done
        let id = progress.id;
        let done = self
            .active
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.transferred_bytes >= t.total_bytes && t.total_bytes > 0)
            .unwrap_or(false);
        if done {
            self.complete(id, None);
        }
    }

    /// Mark a transfer as completed
    pub fn complete(&mut self, id: Uuid, error: Option<String>) {
        if let Some(pos) = self.active.iter().position(|t| t.id == id) {
            let mut item = self.active.remove(pos);
            item.completed_at = Some(Utc::now());

            if let Some(ref err) = error {
                warn!(target: "sftp::transfer", "Transfer {} failed: {}", id, err);
                item.status = TransferStatus::Failed(err.clone());
            } else {
                info!(target: "sftp::transfer", "Transfer {} completed successfully", id);
                item.status = TransferStatus::Completed;
            }

            self.add_to_history(item);
        }
    }

    /// Add to history with size limit
    fn add_to_history(&mut self, item: TransferItem) {
        self.completed.push_front(item);
        while self.completed.len() > self.max_history {
            self.completed.pop_back();
        }
    }

    /// Start next pending transfer if capacity available
    pub fn process_pending(&mut self) -> Vec<TransferItem> {
        let mut started = Vec::new();

        while self.active.len() < self.max_concurrent {
            if let Some(mut item) = self.pending.pop_front() {
                item.status = TransferStatus::InProgress;
                item.started_at = Some(Utc::now());
                started.push(item.clone());
                self.active.push(item);
            } else {
                break;
            }
        }

        started
    }

    /// Get progress sender for transfer tasks
    pub fn progress_sender(&self) -> mpsc::UnboundedSender<TransferProgress> {
        self.progress_tx.clone()
    }

    // process_progress removed - handled by event loop

    /// Clear completed history
    pub fn clear_history(&mut self) {
        self.completed.clear();
    }
}

impl Default for TransferQueue {
    fn default() -> Self {
        Self::new(3) // Default to 3 concurrent transfers
    }
}

/// Worker function to handle transfers
/// Runs in a separate thread/task
pub fn run_transfer_worker(
    host_config: crate::config::HostConfig,
    password: Option<String>,
    mut command_rx: mpsc::UnboundedReceiver<TransferItem>,
    progress_tx: mpsc::UnboundedSender<TransferProgress>,
) {
    info!(target: "sftp::transfer", "Transfer worker starting for {}@{}", host_config.username, host_config.hostname);

    // Connect
    debug!(target: "sftp::transfer", "Establishing SSH connection for transfers");
    let connection_result = crate::ssh::SshConnection::connect_via_proxy(
        host_config.clone(),
        crate::ssh::ProxyConnection::Direct,
        password.as_deref(),
        None,
    );

    let connection = match connection_result {
        Ok(conn) => {
            info!(target: "sftp::transfer", "Transfer worker connected to {}@{}", host_config.username, host_config.hostname);
            conn
        }
        Err(e) => {
            warn!(target: "sftp::transfer", "Transfer worker connection failed: {}", e);
            // Mark all incoming items as failed
            while let Some(item) = command_rx.blocking_recv() {
                let _ = progress_tx.send(TransferProgress {
                    id: item.id,
                    transferred_bytes: 0,
                    speed: 0.0,
                    error: Some(format!("Connection failed: {}", e)),
                });
            }
            return;
        }
    };

    // Open SFTP
    debug!(target: "sftp::transfer", "Opening SFTP subsystem for transfers");
    let sftp = match connection.session_ref().sftp() {
        Ok(s) => {
            info!(target: "sftp::transfer", "SFTP subsystem opened for transfers");
            s
        }
        Err(e) => {
            warn!(target: "sftp::transfer", "Failed to open SFTP subsystem: {}", e);
            while let Some(item) = command_rx.blocking_recv() {
                let _ = progress_tx.send(TransferProgress {
                    id: item.id,
                    transferred_bytes: 0,
                    speed: 0.0,
                    error: Some(format!("SFTP open failed: {}", e)),
                });
            }
            return;
        }
    };

    use std::io::{Read, Write};
    use std::time::Instant;

    while let Some(item) = command_rx.blocking_recv() {
        let direction = match item.direction {
            TransferDirection::Upload => "upload",
            TransferDirection::Download => "download",
        };
        info!(target: "sftp::transfer", "Starting {}: {} -> {} ({} bytes)",
            direction, item.source.display(), item.destination.display(), item.total_bytes);

        // Handle transfer
        let mut transferred = 0u64; // Start from 0 (overwrite)
        let total = item.total_bytes;
        let start_time = Instant::now();
        let mut last_update = Instant::now();

        // Helper to send progress
        let send_progress = |bytes: u64, speed: f64, error: Option<String>| {
            let _ = progress_tx.send(TransferProgress {
                id: item.id,
                transferred_bytes: bytes,
                speed,
                error,
            });
        };

        let result: Result<(), anyhow::Error> = (|| {
            match item.direction {
                TransferDirection::Upload => {
                    let mut local_file = std::fs::File::open(&item.source)?;
                    // Ensure parent directory exists? Sftp::create might fail if parent missing.
                    // For now assume it exists.
                    let mut remote_file = sftp.create(&item.destination)?;

                    let mut buffer = [0u8; 32768]; // 32KB buffer
                    let _file_size = local_file.metadata()?.len();

                    loop {
                        let n = local_file.read(&mut buffer)?;
                        if n == 0 {
                            break;
                        }
                        remote_file.write_all(&buffer[..n])?;
                        transferred += n as u64;

                        // Update progress periodically
                        if last_update.elapsed().as_millis() >= 100 {
                            let duration = start_time.elapsed().as_secs_f64();
                            let speed = if duration > 0.0 {
                                transferred as f64 / duration
                            } else {
                                0.0
                            };
                            send_progress(transferred, speed, None);
                            last_update = Instant::now();
                        }
                    }
                }
                TransferDirection::Download => {
                    let mut remote_file = sftp.open(&item.source)?;
                    let mut local_file = std::fs::File::create(&item.destination)?;

                    let mut buffer = [0u8; 32768];

                    loop {
                        let n = remote_file.read(&mut buffer)?;
                        if n == 0 {
                            break;
                        }
                        local_file.write_all(&buffer[..n])?;
                        transferred += n as u64;

                        if last_update.elapsed().as_millis() >= 100 {
                            let duration = start_time.elapsed().as_secs_f64();
                            let speed = if duration > 0.0 {
                                transferred as f64 / duration
                            } else {
                                0.0
                            };
                            send_progress(transferred, speed, None);
                            last_update = Instant::now();
                        }
                    }
                }
            }
            Ok(())
        })();

        if let Err(e) = result {
            send_progress(transferred, 0.0, Some(e.to_string()));
        } else {
            // Final update (ensure 100%)
            let duration = start_time.elapsed().as_secs_f64();
            let speed = if duration > 0.0 {
                transferred as f64 / duration
            } else {
                0.0
            };
            // Use total_bytes if available to force 100% visualization
            let final_bytes = if total > 0 { total } else { transferred };
            send_progress(final_bytes, speed, None);
        }
    }
}
