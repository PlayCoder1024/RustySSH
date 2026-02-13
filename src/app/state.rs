//! Application state management

use super::{AppEvent, EventHandler};
use crate::config::{Config, HostConfig};
use crate::credentials::CredentialManager;
use crate::sftp::{FileBrowser, SftpSession, SftpSessionManager, TransferQueue};
use crate::ssh::{ConnectionPool, ProxyConnection, SessionManager, SshConnection};
use crate::tui::highlight::Highlighter;
use crate::tui::terminal_render::render_screen_to_lines_with_selection;
use crate::tui::{Icons, Theme, Tui};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Current application view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Connections,
    Session,
    Sftp,
    Tunnels,
    Keys,
    Settings,
    Help,
}

/// Application running state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppState {
    #[default]
    Running,
    Quit,
}

/// Session info for rendering (avoids borrow conflicts)
#[derive(Clone)]
pub struct SessionInfo {
    pub id: Uuid,
    pub name: String,
    /// Styled terminal lines with full color support
    pub styled_lines: Vec<ratatui::text::Line<'static>>,
    pub cursor_position: (u16, u16),
    pub cursor_visible: bool,
    /// Selection state for rendering (normalized start/end positions)
    pub selection: Option<((u16, u16), (u16, u16))>,
    pub status: crate::ssh::SessionStatus,
    pub progress: Option<f32>,
}

/// Render state snapshot (avoids borrow conflicts in draw callback)
#[derive(Clone)]
pub struct RenderState {
    pub view: View,
    pub theme: Theme,
    pub icons: Icons,
    pub config: Config,
    pub highlighter: Highlighter,
    pub sessions: Vec<SessionInfo>,
    pub active_session: Option<Uuid>,
    pub status_message: Option<String>,
    pub selected_host_index: usize,
    pub host_count: usize,
    /// File browser snapshot for SFTP view
    pub file_browser: Option<FileBrowserSnapshot>,
    /// Transfer queue snapshot for SFTP view
    pub transfer_info: TransferQueueSnapshot,
    /// Session order for consistent tab display
    pub session_order: Vec<Uuid>,
    /// Session list overlay visible
    pub session_list_visible: bool,
    /// Selected index in session list overlay
    pub session_list_selected: usize,
    /// Connection overlay visible
    pub show_connection_overlay: bool,
    /// Escape prefix active indicator
    pub escape_prefix_active: bool,
    /// Currently connecting to host (name for loading indicator)
    pub connecting_to_host: Option<String>,
    /// When connection started (for spinner animation timing)
    pub connection_start_time: Option<Instant>,
    /// Find overlay visible
    pub find_overlay_visible: bool,
    /// Find search query
    pub find_query: String,
    /// Current find match index
    pub find_match_index: usize,
    /// Total find matches count
    pub find_match_count: usize,
    /// Host search overlay visible (connections view)
    pub host_search_visible: bool,
    /// Host search query
    pub host_search_query: String,
    /// Host search result indices
    pub host_search_results: Vec<usize>,
    /// Host search selected index
    pub host_search_selected: usize,
    /// Host edit overlay visible (connections view)
    pub host_edit_visible: bool,
    /// Host edit overlay is creating a new host
    pub host_edit_is_new: bool,
    /// Draft host config for edit overlay (new host only)
    pub host_edit_draft: Option<HostConfig>,
    /// Proxy edit overlay visible (connections view)
    pub proxy_edit_visible: bool,
    /// Selected field in proxy edit overlay
    pub proxy_edit_field_index: usize,
    /// Currently editing a proxy field
    pub proxy_editing: bool,
    /// Temporary buffer for proxy editing
    pub proxy_temp_buffer: String,
    /// Tunnel picker overlay visible (connections view)
    pub tunnel_picker_visible: bool,
    /// Selected index in tunnel picker list
    pub tunnel_picker_index: usize,
    /// Selected tunnel names in picker
    pub tunnel_picker_selected: Vec<String>,
    /// Delete confirm overlay visible (connections view)
    pub delete_confirm_visible: bool,
    /// Host id pending deletion confirmation
    pub delete_confirm_host_id: Option<Uuid>,
    /// Settings view category index
    pub settings_category: usize,
    /// Settings view item index within category
    pub settings_item: usize,
    /// Settings dropdown is open
    pub settings_dropdown_open: bool,
    /// Can navigate back
    pub can_go_back: bool,
    /// Can navigate forward
    pub can_go_forward: bool,
    /// Detail view is focused
    pub detail_view_focused: bool,
    /// Selected item index in detail view
    pub detail_view_item_index: usize,
    /// Currently editing a detail field
    pub editing_detail: bool,
    /// Temporary buffer for editing
    pub temp_edit_buffer: String,
    /// Selected tunnel index (tunnels view)
    pub tunnel_selected_index: usize,
    /// Tunnel edit overlay visible (tunnels view)
    pub tunnel_edit_visible: bool,
    /// Tunnel edit overlay is creating a new tunnel
    pub tunnel_edit_is_new: bool,
    /// Draft tunnel config for edit overlay
    pub tunnel_edit_draft: Option<crate::config::TunnelConfig>,
    /// Selected field in tunnel edit overlay
    pub tunnel_edit_field_index: usize,
    /// Currently editing a tunnel field
    pub tunnel_editing: bool,
    /// Temporary buffer for tunnel editing
    pub tunnel_temp_buffer: String,
    /// SSH keys list
    pub ssh_keys: Vec<KeyInfoSnapshot>,
    // Animation state
    pub frame_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProxyKind {
    None,
    JumpHost,
    Socks5,
    Socks4,
    Http,
    ProxyCommand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProxyFieldKind {
    Type,
    Host,
    Address,
    Port,
    Username,
    Password,
    UserId,
    Command,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TunnelFieldKind {
    Name,
    Type,
    AutoStart,
    BindAddr,
    BindPort,
    RemoteHost,
    RemotePort,
    RemoteAddr,
    LocalHost,
    LocalPort,
}

/// Snapshot of a file pane for rendering
#[derive(Clone, Default)]
pub struct FilePaneSnapshot {
    pub path: String,
    pub entries: Vec<FileEntrySnapshot>,
    pub cursor: usize,
    pub is_remote: bool,
}

/// Snapshot of a file entry for rendering
#[derive(Clone)]
pub struct FileEntrySnapshot {
    pub name: String,
    pub is_dir: bool,
    pub size_display: String,
    pub selected: bool,
}

/// Snapshot of file browser for rendering
#[derive(Clone, Default)]
pub struct FileBrowserSnapshot {
    pub left: FilePaneSnapshot,
    pub right: FilePaneSnapshot,
    pub active_is_left: bool,
}

/// Snapshot of transfer queue for rendering
#[derive(Clone, Default)]
pub struct TransferQueueSnapshot {
    pub pending_count: usize,
    pub active_count: usize,
    pub active_transfers: Vec<TransferItemSnapshot>,
}

/// Snapshot of a transfer item
#[derive(Clone)]
pub struct TransferItemSnapshot {
    pub filename: String,
    pub progress: f64,
    pub speed_display: String,
    pub eta_display: String,
    pub is_upload: bool,
}

/// Snapshot of SSH key info for rendering
#[derive(Clone)]
pub struct KeyInfoSnapshot {
    pub name: String,
    pub key_type: String,
    pub fingerprint: String,
    pub comment: String,
    pub encrypted: bool,
    pub path: String,
}

/// Active SSH channel for a session
pub struct ActiveChannel {
    pub session_id: Uuid,
    pub connection_id: Uuid,
    /// Channel for sending data to SSH
    pub input_tx: mpsc::UnboundedSender<Vec<u8>>,
}

/// Active tunnel handle for shutdown
pub struct ActiveTunnel {
    pub name: String,
    pub shutdown: std::sync::mpsc::Sender<()>,
}

/// Main application struct
pub struct App {
    /// Current view
    pub view: View,
    /// Application state
    pub state: AppState,
    /// Configuration
    pub config: Config,
    /// SSH session manager (terminal emulation)
    pub sessions: SessionManager,
    /// SSH connection pool
    pub connections: ConnectionPool,
    /// Active channels mapped by session ID
    pub channels: HashMap<Uuid, ActiveChannel>,
    /// Auto-start tunnels mapped by host ID
    pub autostart_tunnels: HashMap<Uuid, Vec<ActiveTunnel>>,
    /// Active session ID (if in session view)
    pub active_session: Option<Uuid>,
    /// Status message
    pub status_message: Option<String>,
    /// Selected host index in connections view
    pub selected_host_index: usize,
    /// Terminal UI
    tui: Tui,
    /// Event handler
    events: EventHandler,
    /// Theme
    pub theme: Theme,
    /// Icons (Nerd Font or ASCII)
    pub icons: Icons,
    /// Credential manager for secure password storage
    pub credentials: CredentialManager,
    /// SFTP session manager
    pub sftp_sessions: SftpSessionManager,
    /// File browser for SFTP view
    pub file_browser: Option<FileBrowser>,
    /// Transfer queue for SFTP operations
    pub transfer_queue: TransferQueue,
    /// Active transfer workers
    pub transfer_workers: HashMap<Uuid, mpsc::UnboundedSender<crate::sftp::TransferItem>>,
    /// Active SFTP host ID (for which host the SFTP view is showing)
    pub active_sftp_host: Option<Uuid>,
    /// View history stack for back navigation
    pub view_back_history: Vec<View>,
    /// View history stack for forward navigation
    pub view_forward_history: Vec<View>,
    /// Session passwords for SFTP reuse (cleared on disconnect)
    session_passwords: HashMap<Uuid, String>,
    /// Order of sessions for consistent tab display
    pub session_order: Vec<Uuid>,
    /// Escape prefix active (Ctrl+B pressed)
    pub escape_prefix_active: bool,
    /// Time when escape prefix was activated (for timeout)
    pub escape_prefix_time: Option<Instant>,
    /// Session list overlay visibility
    pub session_list_visible: bool,
    /// Selected index in session list overlay
    pub session_list_selected: usize,
    /// Connection overlay visible (for Ctrl+B c)
    pub show_connection_overlay: bool,
    /// Currently connecting to host (name for loading indicator)
    pub connecting_to_host: Option<String>,
    /// When connection attempt started (for spinner animation)
    pub connection_start_time: Option<Instant>,
    /// Pending connection task handle
    pending_connection: Option<
        tokio::task::JoinHandle<
            Result<(SshConnection, std::collections::HashMap<Uuid, String>), anyhow::Error>,
        >,
    >,
    /// Host ID currently being connected
    pending_connection_host_id: Option<Uuid>,
    /// Hosts to save passwords for after successful connection
    pending_hosts_to_save: Vec<Uuid>,
    /// Find overlay visible in session view
    pub find_overlay_visible: bool,
    /// Find search query
    pub find_query: String,
    /// Current find match index (0-based)
    pub find_match_index: usize,
    /// Find matches positions (row, start_col, end_col)
    pub find_matches: Vec<(u16, u16, u16)>,
    /// Terminal content area (for mouse coordinate conversion)
    pub terminal_area: Option<ratatui::layout::Rect>,
    /// Persistent clipboard instance (for Linux X11 persistence)
    pub clipboard: Option<arboard::Clipboard>,
    /// Host search overlay visible (connections view)
    pub host_search_visible: bool,
    /// Host search query
    pub host_search_query: String,
    /// Host search result indices
    pub host_search_results: Vec<usize>,
    /// Host search selected index
    pub host_search_selected: usize,
    /// Host edit overlay visible (connections view)
    pub host_edit_visible: bool,
    /// Host edit overlay is creating a new host
    pub host_edit_is_new: bool,
    /// Draft host config for edit overlay (new host only)
    pub host_edit_draft: Option<HostConfig>,
    /// Default host template for new host comparison
    pub host_edit_default: Option<HostConfig>,
    /// Proxy edit overlay visible (connections view)
    pub proxy_edit_visible: bool,
    /// Selected field in proxy edit overlay
    pub proxy_edit_field_index: usize,
    /// Currently editing a proxy field
    pub proxy_editing: bool,
    /// Temporary buffer for proxy editing
    pub proxy_temp_buffer: String,
    /// Tunnel picker overlay visible (connections view)
    pub tunnel_picker_visible: bool,
    /// Selected index in tunnel picker list
    pub tunnel_picker_index: usize,
    /// Selected tunnel names in picker
    pub tunnel_picker_selected: Vec<String>,
    /// Delete confirm overlay visible (connections view)
    pub delete_confirm_visible: bool,
    /// Host id pending deletion confirmation
    pub delete_confirm_host_id: Option<Uuid>,
    /// Settings view - selected category index
    pub settings_category: usize,
    /// Settings view - selected item within category
    pub settings_item: usize,
    /// Settings view - dropdown is open
    pub settings_dropdown_open: bool,
    /// Persistent terminal highlighter
    pub highlighter: Highlighter,
    /// Detail view is focused
    pub detail_view_focused: bool,
    /// Selected item index in detail view
    pub detail_view_item_index: usize,
    /// Currently editing a detail field
    pub editing_detail: bool,
    /// Temporary buffer for editing
    pub temp_edit_buffer: String,
    /// Selected tunnel index in tunnels view
    pub tunnel_selected_index: usize,
    /// Tunnel edit overlay visible (tunnels view)
    pub tunnel_edit_visible: bool,
    /// Tunnel edit overlay is creating a new tunnel
    pub tunnel_edit_is_new: bool,
    /// Draft tunnel config for edit overlay
    pub tunnel_edit_draft: Option<crate::config::TunnelConfig>,
    /// Default tunnel template for new tunnel comparison
    pub tunnel_edit_default: Option<crate::config::TunnelConfig>,
    /// Selected field in tunnel edit overlay
    pub tunnel_edit_field_index: usize,
    /// Currently editing a tunnel field
    pub tunnel_editing: bool,
    /// Temporary buffer for tunnel editing
    pub tunnel_temp_buffer: String,
    /// SSH Key Manager
    pub key_manager: crate::ssh::KeyManager,
    /// Last time keys were scanned
    pub ssh_keys_last_scan: std::time::Instant,
    /// Last click time for double/triple click detection
    pub last_click_time: Option<std::time::Instant>,
    /// Last click position (terminal row, col)
    pub last_click_pos: Option<(u16, u16)>,
    /// Consecutive click count
    pub click_count: u8,
    /// Frame count for animations
    pub frame_count: usize,
}

impl App {
    /// Create a new application instance
    pub async fn new() -> Result<Self> {
        let config = Config::load().await?;

        // Load theme from config setting
        let theme = match config.settings.ui.theme.as_str() {
            "gruvbox-dark" => crate::tui::gruvbox_dark(),
            "dracula" => crate::tui::dracula(),
            "nord" => crate::tui::nord(),
            _ => Theme::default(), // tokyo-night
        };
        let icons = Icons::detect();
        let tui = Tui::new()?;
        let events = EventHandler::new(Duration::from_millis(50));
        let credentials = CredentialManager::new().await?;

        // Initialize clipboard (ignore error, will retry on use if needed)
        let clipboard = arboard::Clipboard::new().ok();

        // Initialize highlighter
        let highlighter = Highlighter::new(&config.settings.ui.terminal_highlight);

        // Get event sender for bridge (before events is moved into struct)
        let event_sender = events.sender();

        Ok(Self {
            view: View::default(),
            state: AppState::default(),
            config,
            sessions: SessionManager::new(),
            connections: ConnectionPool::new(),
            channels: HashMap::new(),
            autostart_tunnels: HashMap::new(),
            active_session: None,
            status_message: None,
            selected_host_index: 0,
            tui,
            events,
            theme,
            icons,
            credentials,
            sftp_sessions: SftpSessionManager::new(),
            file_browser: None,
            transfer_queue: {
                let mut q = TransferQueue::default();
                // Bridge transfer progress to main event loop
                if let Some(mut rx) = q.take_progress_receiver() {
                    let sender = event_sender.clone();
                    tokio::spawn(async move {
                        while let Some(progress) = rx.recv().await {
                            let _ = sender.send(AppEvent::SftpProgress(progress));
                        }
                    });
                }
                q
            },
            transfer_workers: HashMap::new(),
            active_sftp_host: None,
            view_back_history: Vec::new(),
            view_forward_history: Vec::new(),
            session_passwords: HashMap::new(),
            session_order: Vec::new(),
            escape_prefix_active: false,
            escape_prefix_time: None,
            session_list_visible: false,
            session_list_selected: 0,
            show_connection_overlay: false,
            connecting_to_host: None,
            connection_start_time: None,
            pending_connection: None,
            pending_connection_host_id: None,
            pending_hosts_to_save: Vec::new(),
            find_overlay_visible: false,
            find_query: String::new(),
            find_match_index: 0,
            find_matches: Vec::new(),
            terminal_area: None,
            clipboard,
            host_search_visible: false,
            host_search_query: String::new(),
            host_search_results: Vec::new(),
            host_search_selected: 0,
            host_edit_visible: false,
            host_edit_is_new: false,
            host_edit_draft: None,
            host_edit_default: None,
            proxy_edit_visible: false,
            proxy_edit_field_index: 0,
            proxy_editing: false,
            proxy_temp_buffer: String::new(),
            tunnel_picker_visible: false,
            tunnel_picker_index: 0,
            tunnel_picker_selected: Vec::new(),
            delete_confirm_visible: false,
            delete_confirm_host_id: None,
            settings_category: 0,
            settings_item: 0,
            settings_dropdown_open: false,
            highlighter,
            detail_view_focused: false,
            detail_view_item_index: 0,
            editing_detail: false,
            temp_edit_buffer: String::new(),
            tunnel_selected_index: 0,
            tunnel_edit_visible: false,
            tunnel_edit_is_new: false,
            tunnel_edit_draft: None,
            tunnel_edit_default: None,
            tunnel_edit_field_index: 0,
            tunnel_editing: false,
            tunnel_temp_buffer: String::new(),
            key_manager: crate::ssh::KeyManager::new(),
            ssh_keys_last_scan: std::time::Instant::now(),
            last_click_time: None,
            last_click_pos: None,
            click_count: 0,
            frame_count: 0,
        })
    }

    /// Refresh SSH keys list
    pub fn refresh_keys(&mut self) -> Result<()> {
        let _ = self.key_manager.scan_keys();
        Ok(())
    }

    /// Get all hosts (groups + ungrouped)
    fn all_hosts(&self) -> Vec<&HostConfig> {
        let mut hosts: Vec<&HostConfig> = Vec::new();
        for group in &self.config.groups {
            if group.expanded {
                hosts.extend(group.hosts.iter());
            }
        }
        hosts.extend(self.config.hosts.iter());
        hosts
    }

    /// Get selected host
    fn selected_host(&self) -> Option<&HostConfig> {
        self.all_hosts().get(self.selected_host_index).copied()
    }

    /// Navigate to a new view (clears forward history)
    fn navigate_to(&mut self, new_view: View) {
        // Don't push if same view
        if self.view != new_view {
            self.view_back_history.push(self.view);
            self.view_forward_history.clear();
            self.view = new_view;
        }
    }

    /// Navigate back in history
    fn navigate_back(&mut self) {
        if let Some(prev_view) = self.view_back_history.pop() {
            self.view_forward_history.push(self.view);
            self.view = prev_view;
        } else {
            // Default to connections if no history and we are in a sub-view
            // But if we are already essentially at root (Connections), do nothing or handled by UI
            if self.view != View::Connections && self.view_back_history.is_empty() {
                // Check if we just want to go back to "home" equivalent
                // For now, let's just make sure we don't get stuck if history is empty but we aren't in connections view
                // self.view = View::Connections;
                // Actually the request implies a stack. If stack is empty, we stay.
            }
        }
    }

    /// Navigate forward in history
    fn navigate_forward(&mut self) {
        if let Some(next_view) = self.view_forward_history.pop() {
            self.view_back_history.push(self.view);
            self.view = next_view;
        }
    }

    /// Process pending transfers
    /// Process pending transfers
    pub fn process_transfers(&mut self) {
        // Start new transfers
        let items = self.transfer_queue.process_pending();
        for item in items {
            self.spawn_transfer(item);
        }
    }

    /// Spawn a transfer task
    fn spawn_transfer(&mut self, item: crate::sftp::TransferItem) {
        let host_id = item.host_id;

        // Retrieve host config and password BEFORE borrowing transfer_workers mutably
        let host_config_opt = self
            .all_hosts()
            .iter()
            .find(|h| h.id == host_id)
            .map(|h| (*h).clone());
        let effective_password = if host_config_opt.is_some() {
            let password = self.credentials.get_password(host_id).ok().flatten();
            let session_pwd = self.session_passwords.get(&host_id).cloned();
            session_pwd.or(password)
        } else {
            None
        };

        // Check if we have a worker for this host
        if let std::collections::hash_map::Entry::Vacant(e) = self.transfer_workers.entry(host_id) {
            if let Some(host) = host_config_opt {
                // Need to spawn a new worker
                let (tx, rx) = mpsc::unbounded_channel();
                let progress_tx = self.transfer_queue.progress_sender();

                let host_clone = host.clone();
                let pwd = effective_password.clone();

                // Spawn the worker thread
                std::thread::spawn(move || {
                    crate::sftp::transfer::run_transfer_worker(host_clone, pwd, rx, progress_tx);
                });

                e.insert(tx);
            } else {
                // Host not found? Fail the transfer
                self.transfer_queue
                    .complete(item.id, Some("Host not found".to_string()));
                return;
            }
        }

        // Send item to worker
        if let Some(tx) = self.transfer_workers.get(&host_id) {
            if let Err(_e) = tx.send(item.clone()) {
                // Worker died?
                self.transfer_workers.remove(&host_id);
                // Retry? Or fail? Fail for now.
                self.transfer_queue
                    .complete(item.id, Some("Transfer worker died".to_string()));
            }
        }
    }

    /// Run the main application loop
    pub async fn run(&mut self) -> Result<()> {
        self.tui.enter()?;
        self.events.start();

        let mut should_redraw = true;

        while self.state != AppState::Quit {
            // Initial scan of keys
            if self.ssh_keys_last_scan.elapsed() > Duration::from_secs(60)
                || self.key_manager.list_keys().is_empty()
            {
                let _ = self.key_manager.scan_keys();
                self.ssh_keys_last_scan = std::time::Instant::now();
            }

            if should_redraw {
                // Create render state to avoid borrow conflict
                let render_state = RenderState {
                    view: self.view,
                    theme: self.theme.clone(),
                    icons: self.icons.clone(),
                    config: self.config.clone(),
                    sessions: {
                        self.sessions
                            .list()
                            .iter()
                            .map(|s| {
                                let is_active = Some(s.id) == self.active_session;
                                let styled_lines = if is_active {
                                    let selection = s.get_selection_for_render();
                                    let raw_lines = render_screen_to_lines_with_selection(
                                        s.screen(),
                                        selection,
                                    );
                                    raw_lines
                                        .into_iter()
                                        .map(|line| self.highlighter.highlight_styled_line(line))
                                        .collect()
                                } else {
                                    Vec::new()
                                };

                                SessionInfo {
                                    id: s.id,
                                    name: s.name.clone(),
                                    styled_lines,
                                    cursor_position: s.cursor_position(),
                                    cursor_visible: s.cursor_visible(),
                                    selection: if is_active {
                                        s.get_selection_for_render()
                                    } else {
                                        None
                                    },
                                    status: s.status,
                                    progress: s.progress,
                                }
                            })
                            .collect()
                    },
                    active_session: self.active_session,
                    status_message: self.status_message.clone(),
                    selected_host_index: self.selected_host_index,

                    // Populate other fields...
                    host_count: self.all_hosts().len(),
                    file_browser: self.file_browser.as_ref().map(|browser| {
                        use crate::sftp::PaneSide;
                        FileBrowserSnapshot {
                            left: FilePaneSnapshot {
                                path: browser.left.path.display().to_string(),
                                entries: browser
                                    .left
                                    .filtered_entries()
                                    .iter()
                                    .map(|e| FileEntrySnapshot {
                                        name: e.name.clone(),
                                        is_dir: e.is_dir,
                                        size_display: e.size_display(),
                                        selected: e.selected,
                                    })
                                    .collect(),
                                cursor: browser.left.cursor,
                                is_remote: browser.left.is_remote,
                            },
                            right: FilePaneSnapshot {
                                path: browser.right.path.display().to_string(),
                                entries: browser
                                    .right
                                    .filtered_entries()
                                    .iter()
                                    .map(|e| FileEntrySnapshot {
                                        name: e.name.clone(),
                                        is_dir: e.is_dir,
                                        size_display: e.size_display(),
                                        selected: e.selected,
                                    })
                                    .collect(),
                                cursor: browser.right.cursor,
                                is_remote: browser.right.is_remote,
                            },
                            active_is_left: browser.active == PaneSide::Left,
                        }
                    }),
                    transfer_info: TransferQueueSnapshot {
                        pending_count: self.transfer_queue.pending().len(),
                        active_count: self.transfer_queue.active().len(),
                        active_transfers: self
                            .transfer_queue
                            .active()
                            .iter()
                            .map(|t| {
                                use crate::sftp::TransferDirection;
                                TransferItemSnapshot {
                                    filename: t
                                        .source
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_default(),
                                    progress: t.progress(),
                                    speed_display: t.speed_display(),
                                    eta_display: t.eta_display(),
                                    is_upload: t.direction == TransferDirection::Upload,
                                }
                            })
                            .collect(),
                    },
                    session_order: self.session_order.clone(),
                    session_list_visible: self.session_list_visible,
                    session_list_selected: self.session_list_selected,
                    show_connection_overlay: self.show_connection_overlay,
                    escape_prefix_active: self.escape_prefix_active,
                    connecting_to_host: self.connecting_to_host.clone(),
                    connection_start_time: self.connection_start_time,
                    find_overlay_visible: self.find_overlay_visible,
                    find_query: self.find_query.clone(),
                    find_match_index: self.find_match_index,
                    find_match_count: self.find_matches.len(),
                    host_search_visible: self.host_search_visible,
                    host_search_query: self.host_search_query.clone(),
                    host_search_results: self.host_search_results.clone(),
                    host_search_selected: self.host_search_selected,
                    host_edit_visible: self.host_edit_visible,
                    host_edit_is_new: self.host_edit_is_new,
                    host_edit_draft: self.host_edit_draft.clone(),
                    proxy_edit_visible: self.proxy_edit_visible,
                    proxy_edit_field_index: self.proxy_edit_field_index,
                    proxy_editing: self.proxy_editing,
                    proxy_temp_buffer: self.proxy_temp_buffer.clone(),
                    tunnel_picker_visible: self.tunnel_picker_visible,
                    tunnel_picker_index: self.tunnel_picker_index,
                    tunnel_picker_selected: self.tunnel_picker_selected.clone(),
                    delete_confirm_visible: self.delete_confirm_visible,
                    delete_confirm_host_id: self.delete_confirm_host_id,
                    settings_category: self.settings_category,
                    settings_item: self.settings_item,
                    settings_dropdown_open: self.settings_dropdown_open,
                    can_go_back: !self.view_back_history.is_empty(),
                    can_go_forward: !self.view_forward_history.is_empty(),
                    highlighter: self.highlighter.clone(),
                    detail_view_focused: self.detail_view_focused,
                    detail_view_item_index: self.detail_view_item_index,
                    editing_detail: self.editing_detail,
                    temp_edit_buffer: self.temp_edit_buffer.clone(),
                    tunnel_selected_index: self.tunnel_selected_index,
                    tunnel_edit_visible: self.tunnel_edit_visible,
                    tunnel_edit_is_new: self.tunnel_edit_is_new,
                    tunnel_edit_draft: self.tunnel_edit_draft.clone(),
                    tunnel_edit_field_index: self.tunnel_edit_field_index,
                    tunnel_editing: self.tunnel_editing,
                    tunnel_temp_buffer: self.tunnel_temp_buffer.clone(),
                    ssh_keys: self
                        .key_manager
                        .list_keys()
                        .iter()
                        .map(|k| KeyInfoSnapshot {
                            name: k
                                .path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                            key_type: k.key_type.clone(),
                            fingerprint: k.fingerprint.clone(),
                            comment: k.comment.clone(),
                            encrypted: k.encrypted,
                            path: k.path.to_string_lossy().to_string(),
                        })
                        .collect(),
                    frame_count: self.frame_count,
                };

                // Render UI
                self.tui.draw(|frame| {
                    crate::tui::ui::render_with_state(frame, &render_state);
                })?;

                // Compute terminal area for mouse coordinate conversion
                // This mirrors the layout logic in render_with_state
                if self.view == View::Session {
                    let size = self.tui.size()?;
                    let chunks = ratatui::layout::Layout::default()
                        .direction(ratatui::layout::Direction::Vertical)
                        .constraints([
                            ratatui::layout::Constraint::Min(3),    // Main content
                            ratatui::layout::Constraint::Length(1), // Status bar
                        ])
                        .split(size);

                    // Session view layout: tabs + terminal
                    let session_chunks = ratatui::layout::Layout::default()
                        .direction(ratatui::layout::Direction::Vertical)
                        .constraints([
                            ratatui::layout::Constraint::Length(1), // Tabs
                            ratatui::layout::Constraint::Min(1),    // Terminal
                        ])
                        .split(chunks[0]);

                    // Terminal block inner area (accounting for borders)
                    let terminal_inner = ratatui::layout::Rect {
                        x: session_chunks[1].x + 1,
                        y: session_chunks[1].y + 1,
                        width: session_chunks[1].width.saturating_sub(2),
                        height: session_chunks[1].height.saturating_sub(2),
                    };
                    self.terminal_area = Some(terminal_inner);
                } else {
                    self.terminal_area = None;
                }
                should_redraw = false;
            }

            // Process transfers
            self.process_transfers();

            // Handle events with batching
            if let Some(event) = self.events.next().await {
                if self.handle_event(event).await? {
                    should_redraw = true;
                }

                // Generic batch processing to reduce CPU/latency under load
                let mut events_processed = 0;
                while events_processed < 50 {
                    if let Some(next_event) = self.events.try_next() {
                        if self.handle_event(next_event).await? {
                            should_redraw = true;
                        }
                        events_processed += 1;
                    } else {
                        break;
                    }
                }
            }
        }

        self.tui.exit()?;
        Ok(())
    }

    /// Handle application events
    /// Returns true if a redraw is needed
    async fn handle_event(&mut self, event: AppEvent) -> Result<bool> {
        match event {
            AppEvent::Key(key) => {
                // With DISAMBIGUATE_ESCAPE_CODES enabled, crossterm reports Press/Release/Repeat events.
                // Only handle Press events to avoid duplicate processing and ensure immediate response.
                if key.kind != KeyEventKind::Press {
                    return Ok(false); // No redraw needed for release/repeat events
                }

                // Debug logging for Ctrl+Shift key combinations
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    info!(
                        "Key event: code={:?}, modifiers={:?}, kind={:?}",
                        key.code, key.modifiers, key.kind
                    );
                }

                // Prioritize escape prefix handling (allows holding Ctrl or standard usage)
                if self.view == View::Session && self.escape_prefix_active {
                    self.handle_key(key).await?;
                    return Ok(true);
                }

                // Handle Ctrl+Shift combinations (copy/paste in session)
                // Only Ctrl+Shift+Y/P are supported to avoid conflict with terminal interrupt (Ctrl+C)
                // Note: Some terminals report Ctrl+Shift+Y/P as uppercase 'Y'/'P' with only CONTROL modifier
                if self.view == View::Session && key.modifiers.contains(KeyModifiers::CONTROL) {
                    // Check for Copy: Ctrl+Shift+y/Y (explicit shift) OR Ctrl+Y (uppercase implies shift)
                    let is_copy = (key.modifiers.contains(KeyModifiers::SHIFT)
                        && matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y')))
                        || key.code == KeyCode::Char('Y');

                    // Check for Paste: Ctrl+Shift+i/I (explicit shift) OR Ctrl+I (uppercase implies shift)
                    let is_paste = (key.modifiers.contains(KeyModifiers::SHIFT)
                        && matches!(key.code, KeyCode::Char('i') | KeyCode::Char('I')))
                        || key.code == KeyCode::Char('I');

                    if is_copy {
                        // Copy selected text to clipboard
                        self.copy_selection_to_clipboard();
                        return Ok(true);
                    }
                    if is_paste {
                        // Paste from clipboard
                        self.paste_from_clipboard().await;
                        return Ok(true);
                    }
                }

                // Handle find overlay input
                if self.view == View::Session && self.find_overlay_visible {
                    self.handle_find_overlay_key(key).await?;
                    return Ok(true);
                }

                // Handle Ctrl+C/Q - only quit from non-session views
                // In session view, forward Ctrl+C to remote server
                // Handle Global Navigation: Alt+Left (Back), Alt+Right (Forward)
                // User explicitly requested to override terminal for these keys
                if key.modifiers.contains(KeyModifiers::ALT) {
                    match key.code {
                        KeyCode::Left => {
                            self.navigate_back();
                            return Ok(true);
                        }
                        KeyCode::Right => {
                            self.navigate_forward();
                            return Ok(true);
                        }
                        _ => {}
                    }
                }

                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match key.code {
                        KeyCode::Char('c') => {
                            if self.view == View::Session {
                                // Forward Ctrl+C (0x03) to remote
                                // Note: If selection was active, it was handled above as Copy
                                if let Some(session_id) = self.active_session {
                                    if let Some(channel) = self.channels.get(&session_id) {
                                        let _ = channel.input_tx.send(vec![0x03]);
                                    }
                                }
                            } else {
                                self.state = AppState::Quit;
                            }
                        }
                        KeyCode::Char('q') | KeyCode::Char('Q') => {
                            // Ctrl+Q always quits (escape hatch from session)
                            self.state = AppState::Quit;
                        }
                        KeyCode::Char('b') if self.view == View::Session => {
                            // Ctrl+B is the escape prefix - route to handle_session_key
                            self.handle_key(key).await?;
                        }
                        _ => {
                            // Forward other Ctrl+key combos to session
                            if self.view == View::Session {
                                if let Some(session_id) = self.active_session {
                                    if let Some(channel) = self.channels.get(&session_id) {
                                        let data = self.key_to_bytes(&key);
                                        if !data.is_empty() {
                                            let _ = channel.input_tx.send(data);
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    self.handle_key(key).await?;
                }
                Ok(true)
            }
            AppEvent::Resize(w, h) => {
                // Handle terminal resize
                // Account for: status bar (2), tab bar (2), terminal block borders (2 rows, 2 cols)
                if let Some(session_id) = self.active_session {
                    let adjusted_cols = w.saturating_sub(2);
                    let adjusted_rows = h.saturating_sub(6);
                    self.sessions
                        .resize_session(session_id, adjusted_cols, adjusted_rows)
                        .await?;
                }
                Ok(true)
            }
            AppEvent::SshData { session_id, data } => {
                self.sessions.process_data(session_id, &data).await?;
                Ok(true)
            }
            AppEvent::SshDisconnected { session_id, reason } => {
                self.status_message = Some(format!("Disconnected: {}", reason));
                let host_id = self.sessions.get(session_id).map(|s| s.host_id);
                self.channels.remove(&session_id);
                self.sessions.remove(session_id);
                // Remove from session order
                self.session_order.retain(|&id| id != session_id);

                if let Some(host_id) = host_id {
                    let still_active = self
                        .sessions
                        .list()
                        .iter()
                        .any(|s| s.host_id == host_id);
                    if !still_active {
                        self.stop_autostart_tunnels(host_id);
                    }
                }

                if self.active_session == Some(session_id) {
                    // Switch to another session if available
                    if let Some(&next_session) = self.session_order.first() {
                        self.active_session = Some(next_session);
                    } else {
                        // No remaining sessions, go back to connections
                        self.view = View::Connections;
                        self.active_session = None;
                    }
                }
                Ok(true)
            }
            AppEvent::Error(msg) => {
                self.status_message = Some(format!("Error: {}", msg));
                Ok(true)
            }
            AppEvent::Mouse(mouse) => {
                self.handle_mouse(mouse).await?;
                Ok(true)
            }
            AppEvent::Tick => {
                let mut needs_redraw = false;

                // Show spinner if connecting
                if self.connecting_to_host.is_some() {
                    needs_redraw = true;
                }

                // Increment animation frame
                self.frame_count = self.frame_count.wrapping_add(1);

                // Redraw if any session has progress or connecting (to animate tabs)
                if !needs_redraw {
                    for session in self.sessions.list() {
                        if session.progress.is_some()
                            || session.status == crate::ssh::SessionStatus::Connecting
                        {
                            needs_redraw = true;
                            break;
                        }
                    }
                }

                // Poll pending connection if any
                if let Some(handle) = &mut self.pending_connection {
                    if handle.is_finished() {
                        // Take ownership of the handle by replacing with None
                        let handle = self.pending_connection.take().unwrap();
                        let host_id = self.pending_connection_host_id.take();
                        let host_name = self.connecting_to_host.take().unwrap_or_default();
                        let hosts_to_save = std::mem::take(&mut self.pending_hosts_to_save);
                        self.connection_start_time = None;

                        // Get the result
                        match handle.await {
                            Ok(Ok((connection, passwords_used))) => {
                                // Connection successful - handle it
                                self.handle_connection_success(
                                    host_id.unwrap_or_default(),
                                    host_name,
                                    connection,
                                    passwords_used,
                                    hosts_to_save,
                                )
                                .await?;
                            }
                            Ok(Err(e)) => {
                                // Connection failed
                                self.handle_connection_failure(&e.to_string(), hosts_to_save)
                                    .await;
                            }
                            Err(e) => {
                                // Task panicked or cancelled
                                self.status_message = Some(format!("Connection cancelled: {}", e));
                            }
                        }
                        needs_redraw = true;
                    }
                }

                Ok(needs_redraw)
            }
            // SFTP progress
            AppEvent::SftpProgress(progress) => {
                // Update queue
                self.transfer_queue.update_progress(progress.clone());

                // If this transfer completed or failed, we might want to refresh the view
                // We can check if it's done by looking at the progress status/error
                // Or better, check the queue after update.
                let id = progress.id;
                // Check if it was just moved to history (completed/failed)
                // Accessing history:
                let completed = self
                    .transfer_queue
                    .completed()
                    .iter()
                    .find(|t| t.id == id)
                    .is_some();

                if completed {
                    // Trigger refresh of file browser if visible
                    if let Some(browser) = &mut self.file_browser {
                        // We assume upload/download based on something...
                        // Actually we don't know easily which pane without looking up the item.
                        // But we can just refresh both or active/inactive?
                        // Let's refresh both to be safe and simple.
                        let _ = browser.left.load_local().await;
                        if let Some(sftp) = self
                            .active_sftp_host
                            .and_then(|h| self.sftp_sessions.get_by_host(h))
                        {
                            let _ = browser.right.load_remote(sftp);
                        }
                    }
                }

                Ok(true) // Start redraw
            }
            AppEvent::TunnelStatus { message } => {
                self.status_message = Some(message);
                Ok(true)
            }
            AppEvent::ConnectionResult {
                host_id: _,
                host_name,
                result,
            } => {
                // Legacy event - kept for compatibility
                match result {
                    Ok(_data) => {
                        self.status_message = Some(format!("Connected to {}", host_name));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Connection failed: {}", e));
                    }
                }
                Ok(true)
            }
        }
    }

    /// Handle key events based on current view
    async fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        // View-specific key handling
        match self.view {
            View::Connections => self.handle_connections_key(key).await?,
            View::Session => self.handle_session_key(key).await?,
            View::Sftp => self.handle_sftp_key(key).await?,
            View::Tunnels => self.handle_tunnels_key(key).await?,
            View::Keys => self.handle_keys_key(key).await?,
            View::Settings => self.handle_settings_key(key).await?,
            View::Help => self.handle_help_key(key).await?,
        }
        Ok(())
    }

    /// Handle mouse events for scrolling and selection
    async fn handle_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        use crossterm::event::MouseButton;
        use MouseEventKind::*;

        if self.delete_confirm_visible {
            return Ok(());
        }

        if self.host_edit_visible || self.proxy_edit_visible || self.tunnel_picker_visible {
            return Ok(());
        }

        match mouse.kind {
            // Mouse button down - start selection
            Down(MouseButton::Left) => {
                if self.view == View::Session {
                    if let Some(area) = self.terminal_area {
                        // Check bounds first
                        if mouse.row >= area.y
                            && mouse.row < area.y + area.height
                            && mouse.column >= area.x
                            && mouse.column < area.x + area.width
                        {
                            // Convert mouse coordinates to terminal cell position
                            let term_row = mouse.row.saturating_sub(area.y);
                            let term_col = mouse.column.saturating_sub(area.x);

                            // Check for double/triple click
                            let now = std::time::Instant::now();
                            let is_consecutive = if let (Some(last_time), Some(last_pos)) =
                                (self.last_click_time, self.last_click_pos)
                            {
                                now.duration_since(last_time)
                                    < std::time::Duration::from_millis(500)
                                    && last_pos == (term_row, term_col)
                            } else {
                                false
                            };

                            if is_consecutive {
                                self.click_count += 1;
                            } else {
                                self.click_count = 1;
                            }

                            self.last_click_time = Some(now);
                            self.last_click_pos = Some((term_row, term_col));

                            if let Some(session_id) = self.active_session {
                                if let Some(session) = self.sessions.get_mut(session_id) {
                                    match self.click_count {
                                        1 => session.start_selection(term_row, term_col),
                                        2 => session.select_word_at(term_row, term_col),
                                        3 => session.select_row_at(term_row),
                                        _ => {
                                            self.click_count = 1;
                                            session.start_selection(term_row, term_col);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Mouse drag - update selection
            Drag(MouseButton::Left) => {
                if self.view == View::Session {
                    if let Some(area) = self.terminal_area {
                        // Convert mouse coordinates to terminal cell position
                        // Clamp to valid bounds
                        let term_row = mouse
                            .row
                            .saturating_sub(area.y)
                            .min(area.height.saturating_sub(1));
                        let term_col = mouse
                            .column
                            .saturating_sub(area.x)
                            .min(area.width.saturating_sub(1));

                        if let Some(session_id) = self.active_session {
                            if let Some(session) = self.sessions.get_mut(session_id) {
                                if session.is_selecting {
                                    session.update_selection(term_row, term_col);
                                }
                            }
                        }
                    }
                }
            }
            // Mouse button up - finish selection or handle click
            Up(MouseButton::Left) => {
                // Check for status bar clicks (navigation arrows)
                if let Ok(size) = self.tui.size() {
                    if mouse.row == size.height.saturating_sub(1) {
                        // Status bar is at the bottom
                        // Layout: " < >  ..."
                        // Col 0: " "
                        // Col 1: "<" (Back)
                        // Col 2: " "
                        // Col 3: ">" (Forward)
                        match mouse.column {
                            1 => {
                                self.navigate_back();
                                return Ok(());
                            }
                            3 => {
                                self.navigate_forward();
                                return Ok(());
                            }
                            _ => {}
                        }
                    }
                }

                if self.view == View::Session {
                    if let Some(session_id) = self.active_session {
                        if let Some(session) = self.sessions.get_mut(session_id) {
                            session.finish_selection();
                        }
                    }
                }
            }
            ScrollUp => {
                match self.view {
                    View::Connections => {
                        // Scroll up in host list (move selection up)
                        if self.selected_host_index > 0 {
                            self.selected_host_index -= 1;
                        }
                    }
                    View::Session => {
                        // Scroll up in terminal history
                        if let Some(session_id) = self.active_session {
                            if let Some(session) = self.sessions.get_mut(session_id) {
                                session.scroll_up(3);
                            }
                        }
                    }
                    _ => {}
                }
            }
            ScrollDown => {
                match self.view {
                    View::Connections => {
                        // Scroll down in host list (move selection down)
                        let host_count = self.all_hosts().len();
                        if self.selected_host_index + 1 < host_count {
                            self.selected_host_index += 1;
                        }
                    }
                    View::Session => {
                        // Scroll down in terminal history (towards present)
                        if let Some(session_id) = self.active_session {
                            if let Some(session) = self.sessions.get_mut(session_id) {
                                session.scroll_down(3);
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_connections_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        if self.delete_confirm_visible {
            return self.handle_delete_confirm_key(key).await;
        }

        if self.proxy_edit_visible {
            return self.handle_proxy_edit_key(key).await;
        }

        if self.tunnel_picker_visible {
            return self.handle_tunnel_picker_key(key).await;
        }

        if self.host_edit_visible {
            return self.handle_host_edit_key(key).await;
        }

        // Handle search overlay input first
        if self.host_search_visible {
            return self.handle_host_search_key(key).await;
        }

        // Handle detail view input if focused
        // Handle detail view input if focused
        if self.detail_view_focused {
            if self.handle_detail_view_key(key).await? {
                return Ok(());
            }
            // If not handled, fall through to host list / global keys
        }

        let host_count = self.all_hosts().len();

        match key.code {
            KeyCode::Tab => {
                // Switch focus to detail view
                if host_count > 0 {
                    self.detail_view_focused = true;
                    // Reset if out of bounds (current fields = 6)
                    if self.detail_view_item_index >= 8 {
                        self.detail_view_item_index = 0;
                    }
                }
            }
            KeyCode::Char('?') => self.view = View::Help,
            KeyCode::Char('s') => self.view = View::Settings,
            KeyCode::Char('K') => {
                self.view = View::Settings;
                self.settings_category = 2; // Keys
                self.settings_item = 0;
            }
            KeyCode::Char('t') => self.view = View::Tunnels,
            KeyCode::Char('/') => {
                // Open host search overlay
                self.host_search_visible = true;
                self.host_search_query.clear();
                self.host_search_selected = 0;
                self.update_host_search_results();
            }
            KeyCode::Char('f') => {
                // Open SFTP for selected host
                if let Some(host) = self.selected_host().cloned() {
                    self.open_sftp_for_host(host).await?;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_host_index > 0 {
                    self.selected_host_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_host_index + 1 < host_count {
                    self.selected_host_index += 1;
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.selected_host_index = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                if host_count > 0 {
                    self.selected_host_index = host_count - 1;
                }
            }
            KeyCode::Enter => {
                // Connect to selected host
                if let Some(host) = self.selected_host().cloned() {
                    self.connect_to_host(host).await?;
                }
            }
            KeyCode::Char('n') => {
                // Open new host edit overlay
                self.open_new_host_edit();
            }
            KeyCode::Char('e') => {
                // Open host edit overlay
                if self.selected_host().is_some() {
                    self.host_edit_visible = true;
                    self.host_edit_is_new = false;
                    self.host_edit_draft = None;
                    self.host_edit_default = None;
                    self.detail_view_focused = false;
                    self.detail_view_item_index = 0;
                    self.editing_detail = false;
                    self.temp_edit_buffer.clear();
                } else {
                    self.status_message = Some("No host to edit".to_string());
                }
            }
            KeyCode::Char('E') => {
                // Edit selected host - open config file
                self.edit_config().await?;
            }
            KeyCode::Char('d') => {
                // Open delete confirmation for selected host
                if let Some(host_id) = self.selected_host().map(|host| host.id) {
                    self.delete_confirm_visible = true;
                    self.delete_confirm_host_id = Some(host_id);
                } else {
                    self.status_message = Some("No host to delete".to_string());
                }
            }
            KeyCode::Esc => {
                // Cancel pending connection if any
                if self.pending_connection.is_some() {
                    // Abort the pending connection
                    if let Some(handle) = self.pending_connection.take() {
                        handle.abort();
                    }
                    self.connecting_to_host = None;
                    self.connection_start_time = None;
                    self.pending_connection_host_id = None;
                    self.pending_hosts_to_save.clear();
                    self.status_message = Some("Connection cancelled".to_string());
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle keyboard input in host search overlay
    async fn handle_host_search_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                // Close search overlay
                self.host_search_visible = false;
                self.host_search_query.clear();
                self.host_search_results.clear();
                self.host_search_selected = 0;
            }
            KeyCode::Enter => {
                // Select the highlighted result
                if !self.host_search_results.is_empty() {
                    let selected_idx = self
                        .host_search_results
                        .get(self.host_search_selected)
                        .copied()
                        .unwrap_or(0);
                    self.selected_host_index = selected_idx;
                }
                // Close overlay
                self.host_search_visible = false;
                self.host_search_query.clear();
                self.host_search_results.clear();
                self.host_search_selected = 0;
            }
            KeyCode::Up => {
                // Move selection up
                if self.host_search_selected > 0 {
                    self.host_search_selected -= 1;
                }
            }
            KeyCode::Down => {
                // Move selection down
                if self.host_search_selected + 1 < self.host_search_results.len() {
                    self.host_search_selected += 1;
                }
            }
            KeyCode::Char(c) => {
                self.host_search_query.push(c);
                self.update_host_search_results();
                self.host_search_selected = 0;
            }
            KeyCode::Backspace => {
                self.host_search_query.pop();
                self.update_host_search_results();
                self.host_search_selected = 0;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_host_edit_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        if self.editing_detail {
            match key.code {
                KeyCode::Esc => {
                    self.editing_detail = false;
                    self.temp_edit_buffer.clear();
                }
                KeyCode::Enter => {
                    self.save_detail_edit().await?;
                    self.editing_detail = false;
                    self.temp_edit_buffer.clear();
                }
                KeyCode::Backspace => {
                    self.temp_edit_buffer.pop();
                }
                KeyCode::Char(c) => {
                    self.temp_edit_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.close_host_edit().await?;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.detail_view_item_index > 0 {
                    self.detail_view_item_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.detail_view_item_index < 7 {
                    self.detail_view_item_index += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.start_detail_edit().await?;
            }
            _ => {}
        }
        Ok(())
    }

    fn open_proxy_edit(&mut self) {
        self.proxy_edit_visible = true;
        self.proxy_edit_field_index = 0;
        self.proxy_editing = false;
        self.proxy_temp_buffer.clear();
    }

    fn open_tunnel_picker(&mut self) {
        self.tunnel_picker_visible = true;
        self.tunnel_picker_index = 0;
        let selected = if let Some(host) = self.current_edit_host() {
            let mut names = Vec::new();
            for tunnel in &self.config.tunnels {
                if host.tunnels.iter().any(|t| t.name() == tunnel.name()) {
                    names.push(tunnel.name().to_string());
                }
            }
            names
        } else {
            Vec::new()
        };
        self.tunnel_picker_selected = selected;
    }

    async fn handle_proxy_edit_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        if self.proxy_editing {
            match key.code {
                KeyCode::Esc => {
                    self.proxy_editing = false;
                    self.proxy_temp_buffer.clear();
                }
                KeyCode::Enter => {
                    let kind = self.current_proxy_kind();
                    if let Some(field) = self.proxy_field_kind(kind, self.proxy_edit_field_index) {
                        let value = self.proxy_temp_buffer.clone();
                        self.set_proxy_field_value(kind, field, value).await?;
                    }
                    self.proxy_editing = false;
                    self.proxy_temp_buffer.clear();
                }
                KeyCode::Backspace => {
                    self.proxy_temp_buffer.pop();
                }
                KeyCode::Char(c) => {
                    self.proxy_temp_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        }

        let kind = self.current_proxy_kind();
        let max_fields = self.proxy_field_count(kind);
        if self.proxy_edit_field_index >= max_fields {
            self.proxy_edit_field_index = max_fields.saturating_sub(1);
        }

        match key.code {
            KeyCode::Esc => {
                self.proxy_edit_visible = false;
                self.proxy_editing = false;
                self.proxy_temp_buffer.clear();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.proxy_edit_field_index > 0 {
                    self.proxy_edit_field_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.proxy_edit_field_index + 1 < max_fields {
                    self.proxy_edit_field_index += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.proxy_edit_field_index == 0 {
                    self.cycle_proxy_kind().await?;
                    let new_max = self.proxy_field_count(self.current_proxy_kind());
                    if self.proxy_edit_field_index >= new_max {
                        self.proxy_edit_field_index = new_max.saturating_sub(1);
                    }
                } else if let Some(field) =
                    self.proxy_field_kind(kind, self.proxy_edit_field_index)
                {
                    self.proxy_temp_buffer = self.proxy_field_value(kind, field);
                    self.proxy_editing = true;
                }
            }
            _ => {}
        }

        Ok(())
    }

    async fn handle_tunnel_picker_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        let tunnel_count = self.config.tunnels.len();

        match key.code {
            KeyCode::Esc => {
                self.tunnel_picker_visible = false;
                self.tunnel_picker_index = 0;
                self.tunnel_picker_selected.clear();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.tunnel_picker_index > 0 {
                    self.tunnel_picker_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.tunnel_picker_index + 1 < tunnel_count {
                    self.tunnel_picker_index += 1;
                }
            }
            KeyCode::Char(' ') => {
                if let Some(tunnel) = self.config.tunnels.get(self.tunnel_picker_index) {
                    let name = tunnel.name().to_string();
                    if let Some(pos) = self
                        .tunnel_picker_selected
                        .iter()
                        .position(|t| t == &name)
                    {
                        self.tunnel_picker_selected.remove(pos);
                    } else {
                        self.tunnel_picker_selected.push(name);
                    }
                }
            }
            KeyCode::Enter => {
                let mut selected = Vec::new();
                for tunnel in &self.config.tunnels {
                    if self
                        .tunnel_picker_selected
                        .iter()
                        .any(|t| t == tunnel.name())
                    {
                        selected.push(crate::config::TunnelRef::Name(
                            tunnel.name().to_string(),
                        ));
                    }
                }

                self.modify_edit_host(|host| {
                    host.tunnels = selected;
                });

                if !self.host_edit_is_new {
                    self.config.save().await?;
                }

                self.tunnel_picker_visible = false;
                self.tunnel_picker_index = 0;
                self.tunnel_picker_selected.clear();
            }
            _ => {}
        }

        Ok(())
    }

    fn current_proxy_kind(&self) -> ProxyKind {
        match self.current_edit_host().and_then(|h| h.proxy.as_ref()) {
            Some(crate::config::ProxyConfig::JumpHost { .. }) => ProxyKind::JumpHost,
            Some(crate::config::ProxyConfig::Socks5 { .. }) => ProxyKind::Socks5,
            Some(crate::config::ProxyConfig::Socks4 { .. }) => ProxyKind::Socks4,
            Some(crate::config::ProxyConfig::Http { .. }) => ProxyKind::Http,
            Some(crate::config::ProxyConfig::ProxyCommand { .. }) => ProxyKind::ProxyCommand,
            None => ProxyKind::None,
        }
    }

    fn proxy_field_count(&self, kind: ProxyKind) -> usize {
        match kind {
            ProxyKind::None => 1,
            ProxyKind::JumpHost => 2,
            ProxyKind::Socks5 => 5,
            ProxyKind::Socks4 => 4,
            ProxyKind::Http => 5,
            ProxyKind::ProxyCommand => 2,
        }
    }

    fn proxy_field_kind(&self, kind: ProxyKind, index: usize) -> Option<ProxyFieldKind> {
        let fields = match kind {
            ProxyKind::None => vec![ProxyFieldKind::Type],
            ProxyKind::JumpHost => vec![ProxyFieldKind::Type, ProxyFieldKind::Host],
            ProxyKind::Socks5 => vec![
                ProxyFieldKind::Type,
                ProxyFieldKind::Address,
                ProxyFieldKind::Port,
                ProxyFieldKind::Username,
                ProxyFieldKind::Password,
            ],
            ProxyKind::Socks4 => vec![
                ProxyFieldKind::Type,
                ProxyFieldKind::Address,
                ProxyFieldKind::Port,
                ProxyFieldKind::UserId,
            ],
            ProxyKind::Http => vec![
                ProxyFieldKind::Type,
                ProxyFieldKind::Address,
                ProxyFieldKind::Port,
                ProxyFieldKind::Username,
                ProxyFieldKind::Password,
            ],
            ProxyKind::ProxyCommand => vec![ProxyFieldKind::Type, ProxyFieldKind::Command],
        };

        fields.get(index).copied()
    }

    fn proxy_field_value(&self, kind: ProxyKind, field: ProxyFieldKind) -> String {
        match field {
            ProxyFieldKind::Type => match kind {
                ProxyKind::None => "None".to_string(),
                ProxyKind::JumpHost => "JumpHost".to_string(),
                ProxyKind::Socks5 => "SOCKS5".to_string(),
                ProxyKind::Socks4 => "SOCKS4".to_string(),
                ProxyKind::Http => "HTTP".to_string(),
                ProxyKind::ProxyCommand => "ProxyCommand".to_string(),
            },
            ProxyFieldKind::Host => self
                .current_edit_host()
                .and_then(|h| h.proxy.as_ref())
                .and_then(|p| match p {
                    crate::config::ProxyConfig::JumpHost { host } => Some(match host {
                        crate::config::JumpHostRef::ByUuid(id) => id.to_string(),
                        crate::config::JumpHostRef::ByHostname(name) => name.clone(),
                        crate::config::JumpHostRef::ByName(name) => name.clone(),
                    }),
                    _ => None,
                })
                .unwrap_or_default(),
            ProxyFieldKind::Address => self
                .current_edit_host()
                .and_then(|h| h.proxy.as_ref())
                .and_then(|p| match p {
                    crate::config::ProxyConfig::Socks5 { address, .. } => Some(address.clone()),
                    crate::config::ProxyConfig::Socks4 { address, .. } => Some(address.clone()),
                    crate::config::ProxyConfig::Http { address, .. } => Some(address.clone()),
                    _ => None,
                })
                .unwrap_or_default(),
            ProxyFieldKind::Port => self
                .current_edit_host()
                .and_then(|h| h.proxy.as_ref())
                .and_then(|p| match p {
                    crate::config::ProxyConfig::Socks5 { port, .. } => Some(*port),
                    crate::config::ProxyConfig::Socks4 { port, .. } => Some(*port),
                    crate::config::ProxyConfig::Http { port, .. } => Some(*port),
                    _ => None,
                })
                .map(|p| p.to_string())
                .unwrap_or_default(),
            ProxyFieldKind::Username => self
                .current_edit_host()
                .and_then(|h| h.proxy.as_ref())
                .and_then(|p| match p {
                    crate::config::ProxyConfig::Socks5 { username, .. } => {
                        username.clone()
                    }
                    crate::config::ProxyConfig::Http { username, .. } => username.clone(),
                    _ => None,
                })
                .unwrap_or_default(),
            ProxyFieldKind::Password => self
                .current_edit_host()
                .and_then(|h| h.proxy.as_ref())
                .and_then(|p| match p {
                    crate::config::ProxyConfig::Socks5 { password, .. } => {
                        password.clone()
                    }
                    crate::config::ProxyConfig::Http { password, .. } => password.clone(),
                    _ => None,
                })
                .unwrap_or_default(),
            ProxyFieldKind::UserId => self
                .current_edit_host()
                .and_then(|h| h.proxy.as_ref())
                .and_then(|p| match p {
                    crate::config::ProxyConfig::Socks4 { user_id, .. } => user_id.clone(),
                    _ => None,
                })
                .unwrap_or_default(),
            ProxyFieldKind::Command => self
                .current_edit_host()
                .and_then(|h| h.proxy.as_ref())
                .and_then(|p| match p {
                    crate::config::ProxyConfig::ProxyCommand { command } => {
                        Some(command.clone())
                    }
                    _ => None,
                })
                .unwrap_or_default(),
        }
    }

    async fn cycle_proxy_kind(&mut self) -> Result<()> {
        let next = match self.current_proxy_kind() {
            ProxyKind::None => ProxyKind::JumpHost,
            ProxyKind::JumpHost => ProxyKind::Socks5,
            ProxyKind::Socks5 => ProxyKind::Socks4,
            ProxyKind::Socks4 => ProxyKind::Http,
            ProxyKind::Http => ProxyKind::ProxyCommand,
            ProxyKind::ProxyCommand => ProxyKind::None,
        };

        self.modify_edit_host(|host| {
            host.proxy = match next {
                ProxyKind::None => None,
                ProxyKind::JumpHost => Some(crate::config::ProxyConfig::JumpHost {
                    host: crate::config::JumpHostRef::ByName(String::new()),
                }),
                ProxyKind::Socks5 => Some(crate::config::ProxyConfig::Socks5 {
                    address: "127.0.0.1".to_string(),
                    port: 1080,
                    username: None,
                    password: None,
                }),
                ProxyKind::Socks4 => Some(crate::config::ProxyConfig::Socks4 {
                    address: "127.0.0.1".to_string(),
                    port: 1080,
                    user_id: None,
                }),
                ProxyKind::Http => Some(crate::config::ProxyConfig::Http {
                    address: "127.0.0.1".to_string(),
                    port: 8080,
                    username: None,
                    password: None,
                }),
                ProxyKind::ProxyCommand => Some(crate::config::ProxyConfig::ProxyCommand {
                    command: "nc %h %p".to_string(),
                }),
            };
        });

        if !self.host_edit_is_new {
            self.config.save().await?;
        }

        Ok(())
    }

    async fn set_proxy_field_value(
        &mut self,
        kind: ProxyKind,
        field: ProxyFieldKind,
        value: String,
    ) -> Result<()> {
        self.modify_edit_host(|host| {
            let value = value.trim().to_string();
            match kind {
                ProxyKind::JumpHost => {
                    if let ProxyFieldKind::Host = field {
                        let host_ref = if let Ok(uuid) = uuid::Uuid::parse_str(&value) {
                            crate::config::JumpHostRef::ByUuid(uuid)
                        } else {
                            crate::config::JumpHostRef::ByName(value)
                        };
                        host.proxy = Some(crate::config::ProxyConfig::JumpHost { host: host_ref });
                    }
                }
                ProxyKind::Socks5 => {
                    if let Some(crate::config::ProxyConfig::Socks5 {
                        address,
                        port,
                        username,
                        password,
                    }) = host.proxy.as_mut()
                    {
                        match field {
                            ProxyFieldKind::Address => *address = value,
                            ProxyFieldKind::Port => {
                                if let Ok(p) = value.parse::<u16>() {
                                    *port = p;
                                }
                            }
                            ProxyFieldKind::Username => {
                                *username = if value.is_empty() { None } else { Some(value) }
                            }
                            ProxyFieldKind::Password => {
                                *password = if value.is_empty() { None } else { Some(value) }
                            }
                            _ => {}
                        }
                    }
                }
                ProxyKind::Socks4 => {
                    if let Some(crate::config::ProxyConfig::Socks4 {
                        address,
                        port,
                        user_id,
                    }) = host.proxy.as_mut()
                    {
                        match field {
                            ProxyFieldKind::Address => *address = value,
                            ProxyFieldKind::Port => {
                                if let Ok(p) = value.parse::<u16>() {
                                    *port = p;
                                }
                            }
                            ProxyFieldKind::UserId => {
                                *user_id = if value.is_empty() { None } else { Some(value) }
                            }
                            _ => {}
                        }
                    }
                }
                ProxyKind::Http => {
                    if let Some(crate::config::ProxyConfig::Http {
                        address,
                        port,
                        username,
                        password,
                    }) = host.proxy.as_mut()
                    {
                        match field {
                            ProxyFieldKind::Address => *address = value,
                            ProxyFieldKind::Port => {
                                if let Ok(p) = value.parse::<u16>() {
                                    *port = p;
                                }
                            }
                            ProxyFieldKind::Username => {
                                *username = if value.is_empty() { None } else { Some(value) }
                            }
                            ProxyFieldKind::Password => {
                                *password = if value.is_empty() { None } else { Some(value) }
                            }
                            _ => {}
                        }
                    }
                }
                ProxyKind::ProxyCommand => {
                    if let ProxyFieldKind::Command = field {
                        host.proxy = Some(crate::config::ProxyConfig::ProxyCommand { command: value });
                    }
                }
                ProxyKind::None => {}
            }
        });

        if !self.host_edit_is_new {
            self.config.save().await?;
        }

        Ok(())
    }

    /// Handle keyboard input in delete confirmation overlay
    async fn handle_delete_confirm_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter | KeyCode::Char('y') => {
                if let Some(host_id) = self.delete_confirm_host_id {
                    self.delete_host_by_id(host_id).await?;
                } else {
                    self.delete_selected_host().await?;
                }
                self.delete_confirm_visible = false;
                self.delete_confirm_host_id = None;
            }
            KeyCode::Esc | KeyCode::Char('n') => {
                self.delete_confirm_visible = false;
                self.delete_confirm_host_id = None;
            }
            _ => {}
        }
        Ok(())
    }

    /// Update host search results based on current query
    fn update_host_search_results(&mut self) {
        let query = self.host_search_query.to_lowercase();
        let hosts = self.all_hosts();

        if query.is_empty() {
            // Show all hosts when query is empty
            self.host_search_results = (0..hosts.len()).collect();
        } else {
            // Filter hosts by name, hostname, or username
            self.host_search_results = hosts
                .iter()
                .enumerate()
                .filter(|(_, host)| {
                    host.name.to_lowercase().contains(&query)
                        || host.hostname.to_lowercase().contains(&query)
                        || host.username.to_lowercase().contains(&query)
                })
                .map(|(idx, _)| idx)
                .collect();
        }
    }

    /// Edit configuration file in external editor
    async fn edit_config(&mut self) -> Result<()> {
        let config_path = Config::config_path();

        // Detect available editor
        let editor = match crate::utils::detect_editor() {
            Some(ed) => ed,
            None => {
                self.status_message =
                    Some("No editor found. Set $EDITOR or install nano/vim/vi.".to_string());
                return Ok(());
            }
        };

        // Pause event polling to avoid consuming keystrokes
        self.events.pause();

        // Small delay to let event loop pause
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Exit TUI mode completely
        self.tui.exit()?;

        // Flush any pending output
        let _ = std::io::stdout().flush();

        // Open editor with proper stdio inheritance
        let status = std::process::Command::new(&editor)
            .arg(&config_path)
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();

        // Small delay to let terminal settle
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Re-enter TUI mode
        self.tui.enter()?;

        // Resume event polling
        self.events.resume();

        // Clear and redraw
        self.tui.clear()?;

        match status {
            Ok(s) if s.success() => {
                // Reload config
                self.config = Config::load().await?;
                self.status_message = Some("Config reloaded".to_string());
                // Reset selection if out of bounds
                let host_count = self.all_hosts().len();
                if self.selected_host_index >= host_count && host_count > 0 {
                    self.selected_host_index = host_count - 1;
                }
                self.clamp_tunnel_selected_index();
            }
            Ok(_) => {
                // :q exits with success code 0, so this handles other exits
                self.config = Config::load().await?;
                self.status_message = Some("Config reloaded".to_string());
                self.clamp_tunnel_selected_index();
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to open editor: {}", e));
            }
        }

        Ok(())
    }

    /// Delete the selected host
    async fn delete_selected_host(&mut self) -> Result<()> {
        if self.all_hosts().is_empty() {
            self.status_message = Some("No host to delete".to_string());
            return Ok(());
        }

        if let Some(host_id) = self.selected_host().map(|host| host.id) {
            self.delete_host_by_id(host_id).await?;
        } else {
            self.status_message = Some("No host to delete".to_string());
        }

        Ok(())
    }

    async fn delete_host_by_id(&mut self, host_id: Uuid) -> Result<()> {
        // Check group hosts
        for group in &mut self.config.groups {
            if let Some(idx) = group.hosts.iter().position(|host| host.id == host_id) {
                let removed = group.hosts.remove(idx);
                self.config.save().await?;
                self.status_message = Some(format!("Deleted host: {}", removed.name));
                self.clamp_selected_host_index();
                return Ok(());
            }
        }

        // Check ungrouped hosts
        if let Some(idx) = self.config.hosts.iter().position(|host| host.id == host_id) {
            let removed = self.config.hosts.remove(idx);
            self.config.save().await?;
            self.status_message = Some(format!("Deleted host: {}", removed.name));
            self.clamp_selected_host_index();
            return Ok(());
        }

        self.status_message = Some("Host not found".to_string());
        Ok(())
    }

    fn clamp_selected_host_index(&mut self) {
        let total = self.all_hosts().len();
        if total == 0 {
            self.selected_host_index = 0;
        } else if self.selected_host_index >= total {
            self.selected_host_index = total - 1;
        }
    }

    fn clamp_tunnel_selected_index(&mut self) {
        let total = self.config.tunnels.len();
        if total == 0 {
            self.tunnel_selected_index = 0;
        } else if self.tunnel_selected_index >= total {
            self.tunnel_selected_index = total - 1;
        }
    }

    fn open_new_host_edit(&mut self) {
        let default_host = self.build_default_new_host();
        self.host_edit_visible = true;
        self.host_edit_is_new = true;
        self.host_edit_draft = Some(default_host.clone());
        self.host_edit_default = Some(default_host);
        self.detail_view_focused = false;
        self.detail_view_item_index = 0;
        self.editing_detail = false;
        self.temp_edit_buffer.clear();
    }

    fn build_default_new_host(&self) -> HostConfig {
        let username = whoami::username();
        let host_num = self.config.hosts.len() + 1;
        HostConfig::new(format!("new-host-{}", host_num), "localhost", username)
    }

    async fn close_host_edit(&mut self) -> Result<()> {
        if self.host_edit_is_new {
            if let (Some(draft), Some(default_host)) =
                (self.host_edit_draft.as_ref(), self.host_edit_default.as_ref())
            {
                if !self.host_is_default(draft, default_host) {
                    let new_host = draft.clone();
                    self.config.hosts.push(new_host.clone());
                    self.config.save().await?;
                    self.status_message = Some(format!("Added host: {}", new_host.name));
                    self.selected_host_index = self.all_hosts().len().saturating_sub(1);
                }
            }
        }

        self.host_edit_visible = false;
        self.host_edit_is_new = false;
        self.host_edit_draft = None;
        self.host_edit_default = None;
        self.editing_detail = false;
        self.temp_edit_buffer.clear();

        Ok(())
    }

    fn host_is_default(&self, draft: &HostConfig, default_host: &HostConfig) -> bool {
        draft.name == default_host.name
            && draft.hostname == default_host.hostname
            && draft.port == default_host.port
            && draft.username == default_host.username
            && self.auth_method_eq(&draft.auth, &default_host.auth)
            && draft.remember_password == default_host.remember_password
            && draft.proxy == default_host.proxy
            && draft.tunnels == default_host.tunnels
    }

    fn auth_method_eq(
        &self,
        left: &crate::config::AuthMethod,
        right: &crate::config::AuthMethod,
    ) -> bool {
        use crate::config::AuthMethod;

        match (left, right) {
            (AuthMethod::Password, AuthMethod::Password) => true,
            (AuthMethod::Agent, AuthMethod::Agent) => true,
            (
                AuthMethod::KeyFile {
                    path: left_path,
                    passphrase_required: left_pass,
                },
                AuthMethod::KeyFile {
                    path: right_path,
                    passphrase_required: right_pass,
                },
            ) => left_path == right_path && left_pass == right_pass,
            (
                AuthMethod::Certificate {
                    cert_path: left_cert,
                    key_path: left_key,
                },
                AuthMethod::Certificate {
                    cert_path: right_cert,
                    key_path: right_key,
                },
            ) => left_cert == right_cert && left_key == right_key,
            _ => false,
        }
    }

    /// Connect to a host and open a session
    /// Handles proxy chains (jump hosts) with recursive password prompts
    /// Integrates with credential manager for saved passwords
    async fn connect_to_host(&mut self, host: HostConfig) -> Result<()> {
        info!(target: "app", "Initiating connection to {}", host.name);
        self.status_message = Some(format!("Connecting to {}...", host.name));

        // Get terminal size
        // Account for: status bar (2 lines), tab bar (2 lines), terminal block borders (2 lines top+bottom, 2 cols left+right)
        let size = self.tui.size()?;
        let _cols = size.width.saturating_sub(2) as u32; // Subtract block borders (left + right)
        let _rows = size.height.saturating_sub(6) as u32; // Subtract: status bar (2) + tab bar (2) + block borders (2)

        // Clone host name for later use
        let host_name = host.name.clone();
        let host_id = host.id;

        // Resolve the full proxy chain
        let proxy_chain = self.config.resolve_proxy_chain(&host);
        if proxy_chain.len() > 1 {
            let chain_names: Vec<_> = proxy_chain.iter().map(|h| h.name.as_str()).collect();
            debug!(target: "app", "Proxy chain resolved: {:?}", chain_names);
        }

        // Collect passwords for all hosts in the chain that need password auth
        let mut passwords: std::collections::HashMap<uuid::Uuid, String> =
            std::collections::HashMap::new();
        let mut hosts_to_save: Vec<uuid::Uuid> = Vec::new();

        // Check if any host in the chain needs password auth
        let hosts_needing_password: Vec<_> = proxy_chain
            .iter()
            .filter(|h| matches!(h.auth, crate::config::AuthMethod::Password))
            .collect();

        if !hosts_needing_password.is_empty() {
            // Check if any host wants to remember password (for saving) or has saved password (for retrieval)
            let any_wants_remember = hosts_needing_password.iter().any(|h| h.remember_password);
            let has_any_saved = hosts_needing_password
                .iter()
                .any(|h| h.remember_password && self.credentials.has_saved_password(h.id));

            // Need to setup/unlock master password if:
            // - Any host wants to remember password (for future saving), OR
            // - Any host has a saved password (for retrieval)
            if (any_wants_remember || has_any_saved) && !self.credentials.is_unlocked() {
                // Need to unlock master password first
                if !self.credentials.has_master_password() {
                    // First time setup - prompt to create master password
                    self.events.pause();
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    self.tui.exit()?;

                    println!("\n🔐 First time setup: Create a master password to secure your saved credentials.");
                    println!("   This password encrypts all saved connection passwords.\n");

                    print!("Create master password: ");
                    let _ = std::io::stdout().flush();
                    let master_pwd = match rpassword::read_password() {
                        Ok(p) => p,
                        Err(e) => {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            self.tui.enter()?;
                            self.events.resume();
                            self.tui.clear()?;
                            self.status_message = Some(format!("Failed to read password: {}", e));
                            return Ok(());
                        }
                    };

                    print!("Confirm master password: ");
                    let _ = std::io::stdout().flush();
                    let confirm_pwd = match rpassword::read_password() {
                        Ok(p) => p,
                        Err(e) => {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            self.tui.enter()?;
                            self.events.resume();
                            self.tui.clear()?;
                            self.status_message = Some(format!("Failed to read password: {}", e));
                            return Ok(());
                        }
                    };

                    if master_pwd != confirm_pwd {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        self.tui.enter()?;
                        self.events.resume();
                        self.tui.clear()?;
                        self.status_message = Some("Passwords don't match. Try again.".to_string());
                        return Ok(());
                    }

                    if let Err(e) = self.credentials.setup_master_password(&master_pwd) {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        self.tui.enter()?;
                        self.events.resume();
                        self.tui.clear()?;
                        self.status_message =
                            Some(format!("Failed to setup master password: {}", e));
                        return Ok(());
                    }

                    println!("\n✅ Master password created successfully!\n");
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    self.tui.enter()?;
                    self.events.resume();
                    self.tui.clear()?;
                } else {
                    // Prompt for existing master password
                    self.events.pause();
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    self.tui.exit()?;

                    print!("\n🔐 Master password: ");
                    let _ = std::io::stdout().flush();
                    let master_pwd = match rpassword::read_password() {
                        Ok(p) => p,
                        Err(e) => {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            self.tui.enter()?;
                            self.events.resume();
                            self.tui.clear()?;
                            self.status_message = Some(format!("Failed to read password: {}", e));
                            return Ok(());
                        }
                    };

                    match self.credentials.unlock(&master_pwd) {
                        Ok(true) => {
                            println!("✅ Unlocked\n");
                            std::thread::sleep(std::time::Duration::from_millis(200));
                        }
                        Ok(false) => {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            self.tui.enter()?;
                            self.events.resume();
                            self.tui.clear()?;
                            self.status_message = Some("Incorrect master password".to_string());
                            return Ok(());
                        }
                        Err(e) => {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            self.tui.enter()?;
                            self.events.resume();
                            self.tui.clear()?;
                            self.status_message = Some(format!("Failed to unlock: {}", e));
                            return Ok(());
                        }
                    }

                    self.tui.enter()?;
                    self.events.resume();
                    self.tui.clear()?;
                }
            }

            // Now collect passwords - try saved passwords first
            let mut need_prompt = false;
            for host_config in &hosts_needing_password {
                // Try saved password first if remember_password is enabled
                if host_config.remember_password && self.credentials.is_unlocked() {
                    if let Ok(Some(saved_pwd)) = self.credentials.get_password(host_config.id) {
                        passwords.insert(host_config.id, saved_pwd);
                        continue;
                    }
                }
                need_prompt = true;
            }

            if need_prompt {
                // Exit TUI mode to prompt for passwords
                self.events.pause();
                tokio::time::sleep(Duration::from_millis(50)).await;
                self.tui.exit()?;

                println!(); // Newline for cleaner output

                for host_config in &hosts_needing_password {
                    // Skip if we already have a saved password
                    if passwords.contains_key(&host_config.id) {
                        continue;
                    }

                    // Show context for proxy chain
                    let context = if host_config.id == host.id {
                        "target".to_string()
                    } else {
                        "jump host".to_string()
                    };

                    print!(
                        "Password for {}@{} ({}): ",
                        host_config.username, host_config.hostname, context
                    );
                    let _ = std::io::stdout().flush();

                    match rpassword::read_password() {
                        Ok(pwd) => {
                            passwords.insert(host_config.id, pwd);
                            // Mark for potential save if remember_password is enabled
                            if host_config.remember_password {
                                hosts_to_save.push(host_config.id);
                            }
                        }
                        Err(e) => {
                            // Re-enter TUI mode before returning error
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            self.tui.enter()?;
                            self.events.resume();
                            self.tui.clear()?;
                            self.status_message = Some(format!("Failed to read password: {}", e));
                            return Ok(());
                        }
                    }
                }

                // Re-enter TUI mode
                std::thread::sleep(std::time::Duration::from_millis(100));
                self.tui.enter()?;
                self.events.resume();
                self.tui.clear()?;
            }
        }

        // Set connecting state for UI feedback
        self.connecting_to_host = Some(host_name.clone());
        self.connection_start_time = Some(Instant::now());
        self.pending_connection_host_id = Some(host_id);
        self.pending_hosts_to_save = hosts_to_save;

        // Spawn connection in background (non-blocking)
        let handle = tokio::task::spawn_blocking(move || {
            use crate::config::ProxyConfig;

            // Connect through the proxy chain
            let mut prev_connection: Option<SshConnection> = None;

            for (i, chain_host) in proxy_chain.iter().enumerate() {
                let is_last = i == proxy_chain.len() - 1;
                let password = passwords.get(&chain_host.id).map(|s| s.as_str());

                // Determine the proxy connection type
                let proxy = if let Some(conn) = prev_connection.take() {
                    // We have a previous connection from jump host chain - tunnel through it
                    ProxyConnection::JumpHost {
                        connection: Box::new(conn),
                    }
                } else if is_last {
                    // For the final host, check if it has a non-JumpHost proxy config
                    match &chain_host.proxy {
                        Some(ProxyConfig::Socks5 {
                            address,
                            port,
                            username,
                            password: proxy_pwd,
                        }) => ProxyConnection::Socks5 {
                            address: address.clone(),
                            port: *port,
                            auth: match (username, proxy_pwd) {
                                (Some(u), Some(p)) => Some((u.clone(), p.clone())),
                                _ => None,
                            },
                        },
                        Some(ProxyConfig::Socks4 {
                            address,
                            port,
                            user_id,
                        }) => ProxyConnection::Socks4 {
                            address: address.clone(),
                            port: *port,
                            user_id: user_id.clone(),
                        },
                        Some(ProxyConfig::Http {
                            address,
                            port,
                            username,
                            password: proxy_pwd,
                        }) => ProxyConnection::HttpConnect {
                            address: address.clone(),
                            port: *port,
                            auth: match (username, proxy_pwd) {
                                (Some(u), Some(p)) => Some((u.clone(), p.clone())),
                                _ => None,
                            },
                        },
                        Some(ProxyConfig::ProxyCommand { command }) => {
                            ProxyConnection::ProxyCommand {
                                command: command.clone(),
                                target_host: chain_host.hostname.clone(),
                                target_port: chain_host.port,
                            }
                        }
                        // JumpHost is already handled via proxy_chain, None means direct
                        Some(ProxyConfig::JumpHost { .. }) | None => ProxyConnection::Direct,
                    }
                } else {
                    // Intermediate jump hosts - connect directly (they're part of the chain)
                    ProxyConnection::Direct
                };

                let connection = SshConnection::connect_via_proxy(
                    chain_host.clone(),
                    proxy,
                    password,
                    None, // passphrase
                )?;

                if is_last {
                    return Ok((connection, passwords));
                } else {
                    prev_connection = Some(connection);
                }
            }

            Err(anyhow::anyhow!("Empty proxy chain"))
        });

        // Store handle - will be polled in Tick handler
        self.pending_connection = Some(handle);

        Ok(())
    }

    /// Handle successful connection (called from Tick when connection completes)
    async fn handle_connection_success(
        &mut self,
        host_id: Uuid,
        host_name: String,
        mut connection: SshConnection,
        passwords_used: std::collections::HashMap<Uuid, String>,
        hosts_to_save: Vec<Uuid>,
    ) -> Result<()> {
        info!(target: "app", "Connection successful to {}", host_name);
        let connection_id = connection.id;

        // Get terminal size
        let size = self.tui.size()?;
        let cols = size.width.saturating_sub(2) as u32;
        let rows = size.height.saturating_sub(6) as u32;

        // Save passwords for hosts that were newly entered
        for host_to_save_id in &hosts_to_save {
            if let Some(pwd) = passwords_used.get(host_to_save_id) {
                if self.credentials.is_unlocked() {
                    if let Err(e) = self.credentials.save_password(*host_to_save_id, pwd).await {
                        warn!(target: "app", "Failed to save password: {}", e);
                    }
                }
            }
        }

        let autostart_host = connection.host.clone();

        // Open shell channel
        match connection.open_shell(cols, rows) {
            Ok(channel) => {
                info!(target: "app", "Session created for {}", host_name);
                // Create session for terminal emulation
                let session_id = self.sessions.create_session(
                    host_id,
                    host_name.clone(),
                    cols as u16,
                    rows as u16,
                );

                // Set status to Connected immediately since the connection is already established
                if let Some(session) = self.sessions.get_mut(session_id) {
                    session.status = crate::ssh::SessionStatus::Connected;
                }

                // Set up channel I/O
                let (input_tx, input_rx) = mpsc::unbounded_channel::<Vec<u8>>();

                // Store channel info
                self.channels.insert(
                    session_id,
                    ActiveChannel {
                        session_id,
                        connection_id,
                        input_tx,
                    },
                );

                // Store connection
                self.connections.add(connection);

                // Start auto-start tunnels after connection is established
                self.start_autostart_tunnels(autostart_host, &passwords_used);

                // Spawn task to handle channel I/O
                let event_sender = self.events.sender();
                tokio::task::spawn_blocking(move || {
                    Self::handle_channel_io(channel, session_id, event_sender, input_rx);
                });

                // Switch to session view
                self.active_session = Some(session_id);
                self.session_order.push(session_id);
                // Use navigate_to to ensure connections view is saved in history
                self.navigate_to(View::Session);
                self.status_message = Some(format!("Connected to {}", host_name));

                // Store password for SFTP reuse
                if let Some(pwd) = passwords_used.get(&host_id) {
                    self.session_passwords.insert(host_id, pwd.clone());
                }
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to open shell: {}", e));
            }
        }

        Ok(())
    }

    /// Handle connection failure (called from Tick when connection fails)
    async fn handle_connection_failure(&mut self, error_msg: &str, hosts_to_save: Vec<Uuid>) {
        warn!(target: "app", "Connection failed: {}", error_msg);
        // Connection failed - if we used saved passwords, clear them
        for host_to_save_id in &hosts_to_save {
            if self.credentials.has_saved_password(*host_to_save_id) {
                if let Err(del_err) = self.credentials.delete_password(*host_to_save_id).await {
                    warn!(target: "app", "Failed to delete invalid password: {}", del_err);
                }
            }
        }
        self.status_message = Some(format!("Connection failed: {}", error_msg));
    }

    fn start_autostart_tunnels(
        &mut self,
        host: HostConfig,
        passwords_used: &std::collections::HashMap<Uuid, String>,
    ) {
        if self.autostart_tunnels.contains_key(&host.id) {
            return;
        }

        let tunnels: Vec<crate::config::TunnelConfig> = self
            .config
            .resolve_host_tunnels(&host)
            .into_iter()
            .filter(|t| t.auto_start())
            .cloned()
            .collect();

        if tunnels.is_empty() {
            return;
        }

        let proxy_chain = self.config.resolve_proxy_chain(&host);
        let passwords = passwords_used.clone();
        let mut handles: Vec<ActiveTunnel> = Vec::new();

        for tunnel in tunnels {
            let proxy_chain = proxy_chain.clone();
            let passwords = passwords.clone();
            let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel::<()>();
            let tunnel_name = tunnel.name().to_string();
            let tunnel_name_for_handle = tunnel_name.clone();
            let host_name = host.name.clone();
            let event_sender = self.events.sender();

            tokio::task::spawn_blocking(move || {
                let mut report = |msg: &str| {
                    let _ = event_sender.send(AppEvent::TunnelStatus {
                        message: msg.to_string(),
                    });
                };

                report(&format!(
                    "Starting tunnel: {} ({})",
                    tunnel_name, host_name
                ));

                match App::connect_via_proxy_chain(&proxy_chain, &passwords) {
                    Ok(conn) => {
                        if let Err(e) = crate::ssh::run_tunnel(conn, tunnel, shutdown_rx, &mut report) {
                            report(&format!(
                                "Tunnel failed: {} ({}) - {}",
                                tunnel_name, host_name, e
                            ));
                        } else {
                            report(&format!(
                                "Tunnel stopped: {} ({})",
                                tunnel_name, host_name
                            ));
                        }
                    }
                    Err(e) => {
                        report(&format!(
                            "Tunnel failed: {} ({}) - {}",
                            tunnel_name, host_name, e
                        ));
                    }
                }
            });

            handles.push(ActiveTunnel {
                name: tunnel_name_for_handle,
                shutdown: shutdown_tx,
            });
        }

        if !handles.is_empty() {
            self.autostart_tunnels.insert(host.id, handles);
        }
    }

    fn stop_autostart_tunnels(&mut self, host_id: Uuid) {
        if let Some(handles) = self.autostart_tunnels.remove(&host_id) {
            let count = handles.len();
            for handle in handles {
                let _ = handle.shutdown.send(());
            }
            if count > 0 {
                self.status_message = Some(format!("Stopped {} tunnel(s)", count));
            }
        }
    }

    fn connect_via_proxy_chain(
        proxy_chain: &[HostConfig],
        passwords: &std::collections::HashMap<Uuid, String>,
    ) -> Result<SshConnection> {
        use crate::config::ProxyConfig;

        let mut prev_connection: Option<SshConnection> = None;

        for (i, chain_host) in proxy_chain.iter().enumerate() {
            let is_last = i == proxy_chain.len() - 1;
            let password = passwords.get(&chain_host.id).map(|s| s.as_str());

            let proxy = if let Some(conn) = prev_connection.take() {
                ProxyConnection::JumpHost {
                    connection: Box::new(conn),
                }
            } else if is_last {
                match &chain_host.proxy {
                    Some(ProxyConfig::Socks5 {
                        address,
                        port,
                        username,
                        password: proxy_pwd,
                    }) => ProxyConnection::Socks5 {
                        address: address.clone(),
                        port: *port,
                        auth: match (username, proxy_pwd) {
                            (Some(u), Some(p)) => Some((u.clone(), p.clone())),
                            _ => None,
                        },
                    },
                    Some(ProxyConfig::Socks4 {
                        address,
                        port,
                        user_id,
                    }) => ProxyConnection::Socks4 {
                        address: address.clone(),
                        port: *port,
                        user_id: user_id.clone(),
                    },
                    Some(ProxyConfig::Http {
                        address,
                        port,
                        username,
                        password: proxy_pwd,
                    }) => ProxyConnection::HttpConnect {
                        address: address.clone(),
                        port: *port,
                        auth: match (username, proxy_pwd) {
                            (Some(u), Some(p)) => Some((u.clone(), p.clone())),
                            _ => None,
                        },
                    },
                    Some(ProxyConfig::ProxyCommand { command }) => ProxyConnection::ProxyCommand {
                        command: command.clone(),
                        target_host: chain_host.hostname.clone(),
                        target_port: chain_host.port,
                    },
                    Some(ProxyConfig::JumpHost { .. }) | None => ProxyConnection::Direct,
                }
            } else {
                ProxyConnection::Direct
            };

            let connection =
                SshConnection::connect_via_proxy(chain_host.clone(), proxy, password, None)?;

            if is_last {
                return Ok(connection);
            }

            prev_connection = Some(connection);
        }

        Err(anyhow::anyhow!("Empty proxy chain"))
    }

    /// Open SFTP session for a host, creating a new SSH connection for SFTP
    /// We create a separate connection because ssh2 sessions can't safely
    /// share between blocking SFTP and non-blocking shell I/O
    async fn open_sftp_for_host(&mut self, host: HostConfig) -> Result<()> {
        let host_id = host.id;
        let host_name = host.name.clone();

        // Check if we already have an SFTP session for this host
        if self.sftp_sessions.get_by_host(host_id).is_some() {
            self.active_sftp_host = Some(host_id);
            self.navigate_to(View::Sftp);

            // Initialize file browser if needed
            if self.file_browser.is_none() {
                let mut browser = FileBrowser::new();
                browser.left.load_local().await?;
                self.file_browser = Some(browser);
            }

            // Reload remote pane
            if let Some(browser) = &mut self.file_browser {
                if let Some(sftp_session) = self.sftp_sessions.get_by_host(host_id) {
                    browser.right.path = sftp_session.cwd.clone();
                    let _ = browser.right.load_remote(sftp_session);
                }
            }

            return Ok(());
        }

        // Get password: first check session passwords (from current session), then credential manager
        let password = self.session_passwords.get(&host_id).cloned().or_else(|| {
            if self.credentials.is_unlocked() {
                self.credentials.get_password(host_id).ok().flatten()
            } else {
                None
            }
        });

        // Check if we have a password or if the host uses key/agent authentication
        let has_key_auth = matches!(
            host.auth,
            crate::config::AuthMethod::KeyFile { .. }
                | crate::config::AuthMethod::Agent
                | crate::config::AuthMethod::Certificate { .. }
        );
        if password.is_none() && !has_key_auth {
            self.status_message = Some(format!(
                "No password available for SFTP. Reconnect to {} first.",
                host_name
            ));
            return Ok(());
        }

        // Create a new connection for SFTP (separate from shell connection)
        // Must use proxy chain just like connect_to_host does
        let pwd_source = if self.session_passwords.contains_key(&host_id) {
            "session"
        } else if password.is_some() {
            "credential manager"
        } else {
            "none"
        };
        let has_pwd = password.is_some();

        // Resolve the full proxy chain (same as connect_to_host)
        let proxy_chain = self.config.resolve_proxy_chain(&host);

        // Collect passwords for all hosts in the chain
        let mut passwords: std::collections::HashMap<uuid::Uuid, String> =
            std::collections::HashMap::new();

        // Get passwords for all hosts in the chain
        for chain_host in &proxy_chain {
            // First check session passwords
            if let Some(pwd) = self.session_passwords.get(&chain_host.id) {
                passwords.insert(chain_host.id, pwd.clone());
            } else if self.credentials.is_unlocked() {
                // Try credential manager
                if let Ok(Some(pwd)) = self.credentials.get_password(chain_host.id) {
                    passwords.insert(chain_host.id, pwd);
                }
            }
        }

        // For the target host, also try the password variable we already retrieved
        if let Some(pwd) = password.clone() {
            passwords.entry(host_id).or_insert(pwd);
        }

        let username = host.username.clone();
        let sftp_result = tokio::task::spawn_blocking(move || {
            use crate::config::ProxyConfig;

            // Connect through the proxy chain (same logic as connect_to_host)
            let mut prev_connection: Option<SshConnection> = None;

            for (i, chain_host) in proxy_chain.iter().enumerate() {
                let is_last = i == proxy_chain.len() - 1;
                let password = passwords.get(&chain_host.id).map(|s| s.as_str());

                // Determine the proxy connection type (same logic as connect_to_host)
                let proxy = if let Some(conn) = prev_connection.take() {
                    ProxyConnection::JumpHost {
                        connection: Box::new(conn),
                    }
                } else if is_last {
                    // For the final host, check if it has a non-JumpHost proxy config
                    match &chain_host.proxy {
                        Some(ProxyConfig::Socks5 {
                            address,
                            port,
                            username,
                            password: proxy_pwd,
                        }) => ProxyConnection::Socks5 {
                            address: address.clone(),
                            port: *port,
                            auth: match (username, proxy_pwd) {
                                (Some(u), Some(p)) => Some((u.clone(), p.clone())),
                                _ => None,
                            },
                        },
                        Some(ProxyConfig::Socks4 {
                            address,
                            port,
                            user_id,
                        }) => ProxyConnection::Socks4 {
                            address: address.clone(),
                            port: *port,
                            user_id: user_id.clone(),
                        },
                        Some(ProxyConfig::Http {
                            address,
                            port,
                            username,
                            password: proxy_pwd,
                        }) => ProxyConnection::HttpConnect {
                            address: address.clone(),
                            port: *port,
                            auth: match (username, proxy_pwd) {
                                (Some(u), Some(p)) => Some((u.clone(), p.clone())),
                                _ => None,
                            },
                        },
                        Some(ProxyConfig::ProxyCommand { command }) => {
                            ProxyConnection::ProxyCommand {
                                command: command.clone(),
                                target_host: chain_host.hostname.clone(),
                                target_port: chain_host.port,
                            }
                        }
                        Some(ProxyConfig::JumpHost { .. }) | None => ProxyConnection::Direct,
                    }
                } else {
                    ProxyConnection::Direct
                };

                let connection =
                    SshConnection::connect_via_proxy(chain_host.clone(), proxy, password, None)?;

                if is_last {
                    // Open SFTP
                    let sftp = match connection.session_ref().sftp() {
                        Ok(s) => s,
                        Err(e) => {
                            return Err(anyhow::anyhow!("Failed to open SFTP session: {}", e));
                        }
                    };
                    let conn_id = connection.id;

                    // Create SFTP session with blocking calls INSIDE spawn_blocking
                    let sftp_session = SftpSession::new(sftp, host_id, conn_id, &username)?;
                    let session_cwd = sftp_session.cwd.clone();

                    // Load initial directory listing (blocking) INSIDE spawn_blocking
                    let entries = sftp_session.read_dir(&session_cwd).unwrap_or_default();

                    return Ok::<_, anyhow::Error>((connection, sftp_session, entries));
                } else {
                    prev_connection = Some(connection);
                }
            }

            Err(anyhow::anyhow!("Empty proxy chain"))
        })
        .await?;

        match sftp_result {
            Ok((connection, sftp_session, initial_entries)) => {
                // Store connection
                self.connections.add(connection);

                // Store SFTP session (already created in blocking context)
                let session_cwd = sftp_session.cwd.clone();
                self.sftp_sessions.add(sftp_session);
                self.active_sftp_host = Some(host_id);

                // Initialize file browser with pre-loaded entries
                let mut browser = if let Some(b) = self.file_browser.take() {
                    b
                } else {
                    FileBrowser::new()
                };

                browser.left.load_local().await?;
                browser.right.path = session_cwd;
                browser.right.entries = initial_entries;
                browser.right.sort_entries();

                self.file_browser = Some(browser);
                self.navigate_to(View::Sftp);
                self.status_message = Some(format!("SFTP connected to {}", host_name));
            }
            Err(e) => {
                self.status_message = Some(format!(
                    "SFTP failed (pwd from {}, has_pwd={}): {}",
                    pwd_source, has_pwd, e
                ));
            }
        }

        Ok(())
    }

    /// Handle channel I/O in a blocking context
    fn handle_channel_io(
        mut channel: ssh2::Channel,
        session_id: Uuid,
        event_sender: mpsc::UnboundedSender<AppEvent>,
        mut input_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    ) {
        use std::io::Write;

        // Note: ssh2 blocking is controlled at Session level
        // We'll use short reads with timeout

        // usage 16KB buffer for better throughput
        let mut buf = [0u8; 16384];

        loop {
            // Check if channel is closed
            if channel.eof() {
                let _ = event_sender.send(AppEvent::SshDisconnected {
                    session_id,
                    reason: "Channel closed".to_string(),
                });
                break;
            }

            let mut did_work = false;

            // Read available data
            match channel.read(&mut buf) {
                Ok(0) => {
                    // No data available, check for input
                }
                Ok(n) => {
                    did_work = true;
                    let data = buf[..n].to_vec();
                    if event_sender
                        .send(AppEvent::SshData { session_id, data })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data available
                }
                Err(e) => {
                    let _ = event_sender.send(AppEvent::SshDisconnected {
                        session_id,
                        reason: format!("Read error: {}", e),
                    });
                    break;
                }
            }

            // Check for input to send - drain the queue
            let mut inputs_processed = 0;
            loop {
                match input_rx.try_recv() {
                    Ok(data) => {
                        did_work = true;
                        if let Err(e) = channel.write_all(&data) {
                            let _ = event_sender.send(AppEvent::SshDisconnected {
                                session_id,
                                reason: format!("Write error: {}", e),
                            });
                            return;
                        }
                        inputs_processed += 1;
                        // Don't starve reads if we have massive input (though unlikely for manual input)
                        if inputs_processed > 100 {
                            break;
                        }
                    }
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => return,
                }
            }

            if inputs_processed > 0 {
                let _ = channel.flush();
            }

            // Small sleep ONLY if no work was done to prevent busy loop
            // If we did work, we continue immediately to process more data
            if !did_work {
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        let _ = channel.close();
    }

    async fn handle_session_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        // Escape prefix timeout (1 second)
        const ESCAPE_PREFIX_TIMEOUT: Duration = Duration::from_secs(1);

        // Check for escape prefix timeout
        if self.escape_prefix_active {
            if let Some(prefix_time) = self.escape_prefix_time {
                if prefix_time.elapsed() > ESCAPE_PREFIX_TIMEOUT {
                    self.escape_prefix_active = false;
                    self.escape_prefix_time = None;
                }
            }
        }

        // Handle session list overlay if visible
        if self.session_list_visible {
            return self.handle_session_list_key(key).await;
        }

        // Handle connection overlay if visible
        if self.show_connection_overlay {
            return self.handle_connection_overlay_key(key).await;
        }

        // Check for Ctrl+B (escape prefix)
        if key.code == KeyCode::Char('b') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if self.escape_prefix_active {
                // Double Ctrl+B: send literal Ctrl+B to host
                self.escape_prefix_active = false;
                self.escape_prefix_time = None;
                if let Some(session_id) = self.active_session {
                    if let Some(channel) = self.channels.get(&session_id) {
                        let _ = channel.input_tx.send(vec![0x02]); // Ctrl+B = 0x02
                    }
                }
            } else {
                // Enter escape prefix mode
                self.escape_prefix_active = true;
                self.escape_prefix_time = Some(Instant::now());
                self.status_message = Some("Ctrl+B".to_string());
            }
            return Ok(());
        }

        // Handle escape prefix commands
        if self.escape_prefix_active {
            self.escape_prefix_active = false;
            self.escape_prefix_time = None;
            self.status_message = None;

            match key.code {
                // n - Next session
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.switch_to_next_session();
                    return Ok(());
                }
                // p - Previous session
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    self.switch_to_prev_session();
                    return Ok(());
                }
                // l - Show session list
                KeyCode::Char('l') | KeyCode::Char('L') => {
                    self.session_list_visible = true;
                    self.session_list_selected = self.get_active_session_index();
                    return Ok(());
                }
                // c - New connection (show connection overlay)
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    self.show_connection_overlay = true;
                    return Ok(());
                }
                // w - Close current session
                KeyCode::Char('w') | KeyCode::Char('W') => {
                    self.close_current_session().await;
                    return Ok(());
                }
                // f - Find in session
                KeyCode::Char('f') | KeyCode::Char('F') => {
                    self.find_overlay_visible = true;
                    self.find_query.clear();
                    self.find_matches.clear();
                    self.find_match_index = 0;
                    return Ok(());
                }
                // Any other key: forward to host (prefix was accidental)
                _ => {
                    // Fall through to normal key handling
                }
            }
        }

        // Check for Alt+f to open SFTP for current session's host
        if key.code == KeyCode::Char('f') && key.modifiers.contains(KeyModifiers::ALT) {
            // Get the host ID from the current session
            if let Some(session_id) = self.active_session {
                if let Some(session) = self.sessions.get(session_id) {
                    let host_id = session.host_id;
                    // Find the host config
                    if let Some(host) = self.all_hosts().iter().find(|h| h.id == host_id).cloned() {
                        self.open_sftp_for_host(host.clone()).await?;
                        return Ok(());
                    }
                }
            }
            self.status_message = Some("No active session for SFTP".to_string());
            return Ok(());
        }

        // Forward key to SSH session
        if let Some(session_id) = self.active_session {
            if let Some(channel) = self.channels.get(&session_id) {
                let data = self.key_to_bytes(&key);
                if !data.is_empty() {
                    let _ = channel.input_tx.send(data);
                }
            }
        }

        Ok(())
    }

    /// Handle keys when session list overlay is visible
    async fn handle_session_list_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('l') => {
                self.session_list_visible = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.session_list_selected > 0 {
                    self.session_list_selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.session_list_selected + 1 < self.session_order.len() {
                    self.session_list_selected += 1;
                }
            }
            KeyCode::Enter => {
                self.switch_to_session_index(self.session_list_selected);
                self.session_list_visible = false;
            }
            KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                let index = (c as usize) - ('1' as usize);
                self.switch_to_session_index(index);
                self.session_list_visible = false;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle keys when connection overlay is visible
    async fn handle_connection_overlay_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.show_connection_overlay = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_host_index > 0 {
                    self.selected_host_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let host_count = self.all_hosts().len();
                if self.selected_host_index + 1 < host_count {
                    self.selected_host_index += 1;
                }
            }
            KeyCode::Enter => {
                // Connect to selected host while keeping current sessions
                if let Some(host) = self.selected_host().cloned() {
                    self.show_connection_overlay = false;
                    self.connect_to_host(host).await?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Get index of active session in session_order
    fn get_active_session_index(&self) -> usize {
        if let Some(active_id) = self.active_session {
            self.session_order
                .iter()
                .position(|&id| id == active_id)
                .unwrap_or(0)
        } else {
            0
        }
    }

    /// Switch to next session
    fn switch_to_next_session(&mut self) {
        if self.session_order.is_empty() {
            return;
        }
        let current_index = self.get_active_session_index();
        let next_index = (current_index + 1) % self.session_order.len();
        self.switch_to_session_index(next_index);
    }

    /// Switch to previous session
    fn switch_to_prev_session(&mut self) {
        if self.session_order.is_empty() {
            return;
        }
        let current_index = self.get_active_session_index();
        let prev_index = if current_index == 0 {
            self.session_order.len() - 1
        } else {
            current_index - 1
        };
        self.switch_to_session_index(prev_index);
    }

    /// Switch to session by index
    fn switch_to_session_index(&mut self, index: usize) {
        if index < self.session_order.len() {
            self.active_session = Some(self.session_order[index]);
            self.status_message = Some(format!("Session {}", index + 1));
        }
    }

    /// Close current session
    async fn close_current_session(&mut self) {
        if let Some(session_id) = self.active_session {
            // Remove from session order
            self.session_order.retain(|&id| id != session_id);

            // Remove channel and session
            self.channels.remove(&session_id);
            self.sessions.remove(session_id);

            // Switch to another session or go back to connections
            if let Some(&next_session) = self.session_order.first() {
                self.active_session = Some(next_session);
                self.status_message = Some("Session closed".to_string());
            } else {
                self.active_session = None;
                self.view = View::Connections;
                self.status_message = Some("All sessions closed".to_string());
            }
        }
    }

    async fn handle_sftp_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        // Handle SFTP-specific keys
        match key.code {
            // Navigation keys
            KeyCode::Esc => {
                self.navigate_back();
            }
            KeyCode::Char('?') => self.view = View::Help,

            // Pane switching
            KeyCode::Tab => {
                if let Some(browser) = &mut self.file_browser {
                    browser.switch_pane();
                }
            }

            // Cursor movement
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(browser) = &mut self.file_browser {
                    browser.active_pane_mut().cursor_up();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(browser) = &mut self.file_browser {
                    browser.active_pane_mut().cursor_down();
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                if let Some(browser) = &mut self.file_browser {
                    browser.active_pane_mut().cursor_top();
                }
            }
            KeyCode::End | KeyCode::Char('G') => {
                if let Some(browser) = &mut self.file_browser {
                    browser.active_pane_mut().cursor_bottom();
                }
            }
            KeyCode::PageUp => {
                if let Some(browser) = &mut self.file_browser {
                    browser.active_pane_mut().page_up(10);
                }
            }
            KeyCode::PageDown => {
                if let Some(browser) = &mut self.file_browser {
                    browser.active_pane_mut().page_down(10);
                }
            }

            // Selection
            KeyCode::Char(' ') => {
                if let Some(browser) = &mut self.file_browser {
                    browser.active_pane_mut().toggle_selection();
                    browser.active_pane_mut().cursor_down();
                }
            }

            // Enter directory or trigger transfer
            KeyCode::Enter => {
                self.handle_sftp_enter().await?;
            }

            // Backspace goes up a directory
            KeyCode::Backspace => {
                self.handle_sftp_go_parent().await?;
            }

            // Copy to other pane (F5 or 'c')
            KeyCode::F(5) | KeyCode::Char('c') => {
                self.handle_sftp_copy().await?;
            }

            // Move to other pane (F6 or 'm')
            KeyCode::F(6) | KeyCode::Char('m') => {
                self.handle_sftp_move().await?;
            }

            // Delete (F8 or 'd') - local only
            KeyCode::F(8) | KeyCode::Char('d') => {
                self.handle_sftp_delete().await?;
            }

            // New directory (n)
            KeyCode::Char('n') => {
                self.status_message = Some("New directory: not yet implemented".to_string());
            }

            // Rename (r)
            KeyCode::Char('r') => {
                self.status_message = Some("Rename: not yet implemented".to_string());
            }

            // Toggle hidden files (h)
            KeyCode::Char('h') => {
                if let Some(browser) = &mut self.file_browser {
                    browser.active_pane_mut().toggle_hidden();
                }
            }

            // Sort cycling (s)
            KeyCode::Char('s') => {
                if let Some(browser) = &mut self.file_browser {
                    browser.active_pane_mut().cycle_sort();
                }
            }

            // Refresh (F2)
            KeyCode::F(2) => {
                self.handle_sftp_refresh().await?;
            }

            // Quit SFTP connection (q)
            KeyCode::Char('q') => {
                if let Some(host_id) = self.active_sftp_host {
                    self.sftp_sessions.remove_by_host(host_id);
                    self.active_sftp_host = None;
                    self.status_message = Some("SFTP connection closed".to_string());

                    // Remove all SFTP entries from history to prevent navigating back to a closed session
                    self.view_back_history.retain(|&v| v != View::Sftp);
                    self.view_forward_history.retain(|&v| v != View::Sftp);

                    // Navigate back without pushing current View::Sftp to history
                    if let Some(prev_view) = self.view_back_history.pop() {
                        self.view = prev_view;
                    } else {
                        self.view = View::Connections;
                    }
                }
            }

            _ => {}
        }
        Ok(())
    }

    /// Handle Enter key in SFTP view - navigate into directory or start transfer
    async fn handle_sftp_enter(&mut self) -> Result<()> {
        let entry_info = if let Some(browser) = &self.file_browser {
            browser
                .active_pane()
                .current_entry()
                .map(|e| (e.is_dir, e.path.clone()))
        } else {
            None
        };

        if let Some((is_dir, path)) = entry_info {
            if is_dir {
                // Navigate into directory
                if let Some(browser) = &mut self.file_browser {
                    let is_remote = browser.active_pane().is_remote;
                    browser.active_pane_mut().path = path;
                    browser.active_pane_mut().cursor = 0;

                    if is_remote {
                        // Reload remote pane
                        if let Some(host_id) = self.active_sftp_host {
                            if let Some(sftp_session) = self.sftp_sessions.get_by_host(host_id) {
                                let path = browser.active_pane().path.clone();
                                if let Err(e) = browser.active_pane_mut().load_remote(sftp_session)
                                {
                                    self.status_message =
                                        Some(format!("Failed to load {}: {}", path.display(), e));
                                }
                            }
                        }
                    } else {
                        // Reload local pane
                        if let Err(e) = browser.active_pane_mut().load_local().await {
                            self.status_message = Some(format!("Failed to load directory: {}", e));
                        }
                    }
                }
            } else {
                // File selected - initiate transfer
                self.handle_sftp_copy().await?;
            }
        }
        Ok(())
    }

    /// Go to parent directory in SFTP view
    async fn handle_sftp_go_parent(&mut self) -> Result<()> {
        let child_name = if let Some(browser) = &mut self.file_browser {
            browser.active_pane_mut().go_parent()
        } else {
            None
        };

        if let Some(name) = child_name {
            self.handle_sftp_refresh().await?;
            if let Some(browser) = &mut self.file_browser {
                browser.active_pane_mut().set_cursor_by_name(&name);
            }
        }
        Ok(())
    }

    /// Refresh current directory in SFTP view
    async fn handle_sftp_refresh(&mut self) -> Result<()> {
        if let Some(browser) = &mut self.file_browser {
            let is_remote = browser.active_pane().is_remote;

            if is_remote {
                if let Some(host_id) = self.active_sftp_host {
                    if let Some(sftp_session) = self.sftp_sessions.get_by_host(host_id) {
                        if let Err(e) = browser.active_pane_mut().load_remote(sftp_session) {
                            self.status_message = Some(format!("Refresh failed: {}", e));
                        }
                    }
                }
            } else {
                if let Err(e) = browser.active_pane_mut().load_local().await {
                    self.status_message = Some(format!("Refresh failed: {}", e));
                }
            }
        }
        Ok(())
    }

    /// Handle copy operation in SFTP view
    async fn handle_sftp_copy(&mut self) -> Result<()> {
        // Get selected files from active pane
        let (source_files, is_upload) = if let Some(browser) = &self.file_browser {
            let active = browser.active_pane();
            let selected = active.selected_entries();

            // If no selections, use current entry
            let files: Vec<_> = if selected.is_empty() {
                active
                    .current_entry()
                    .filter(|e| e.name != "..")
                    .into_iter()
                    .collect()
            } else {
                selected.into_iter().filter(|e| e.name != "..").collect()
            };

            let is_upload = !active.is_remote; // Uploading if source is local
            (
                files
                    .iter()
                    .map(|f| (f.path.clone(), f.size))
                    .collect::<Vec<_>>(),
                is_upload,
            )
        } else {
            return Ok(());
        };

        if source_files.is_empty() {
            self.status_message = Some("No files selected".to_string());
            return Ok(());
        }

        // Get SFTP host and destination path
        let host_id = match self.active_sftp_host {
            Some(id) => id,
            None => {
                self.status_message = Some("No active SFTP connection".to_string());
                return Ok(());
            }
        };

        let dest_path = self
            .file_browser
            .as_ref()
            .map(|b| b.inactive_pane().path.clone())
            .unwrap_or_default();

        let direction = if is_upload {
            crate::sftp::TransferDirection::Upload
        } else {
            crate::sftp::TransferDirection::Download
        };

        let _file_count = source_files.len();
        let mut added_count = 0;

        for (source, size) in source_files {
            let filename = source
                .file_name()
                .map(|n| n.to_os_string())
                .unwrap_or_default();
            let dest = dest_path.join(&filename);

            // Create transfer item
            let item = crate::sftp::TransferItem::new(source, dest, direction, size, host_id);

            // Add to queue
            self.transfer_queue.add(item);
            added_count += 1;
        }

        // Trigger processing immediately
        self.process_transfers();

        self.status_message = Some(format!("Queued {} files for transfer", added_count));

        // Reload directory to show new files (eventually) - but this might be premature.
        // We probably want to auto-refresh when transfer completes.
        Ok(())
    }

    /// Handle move operation in SFTP view
    async fn handle_sftp_move(&mut self) -> Result<()> {
        // For now, same as copy but we would mark for deletion after
        self.status_message = Some("Move: not yet implemented (use copy + delete)".to_string());
        Ok(())
    }

    /// Handle delete operation in SFTP view - LOCAL ONLY for safety
    async fn handle_sftp_delete(&mut self) -> Result<()> {
        if let Some(browser) = &self.file_browser {
            if browser.active_pane().is_remote {
                self.status_message = Some("Delete disabled on remote for safety".to_string());
                return Ok(());
            }
        }

        // Get selected files from active pane (local only)
        let files_to_delete: Vec<_> = if let Some(browser) = &self.file_browser {
            let active = browser.active_pane();
            let selected = active.selected_entries();

            if selected.is_empty() {
                active
                    .current_entry()
                    .filter(|e| e.name != "..")
                    .into_iter()
                    .map(|e| e.path.clone())
                    .collect()
            } else {
                selected
                    .into_iter()
                    .filter(|e| e.name != "..")
                    .map(|e| e.path.clone())
                    .collect()
            }
        } else {
            return Ok(());
        };

        if files_to_delete.is_empty() {
            self.status_message = Some("No files selected for deletion".to_string());
            return Ok(());
        }

        // TODO: Add confirmation prompt
        self.status_message = Some(format!(
            "Delete {} file(s): confirmation not yet implemented",
            files_to_delete.len()
        ));
        Ok(())
    }

    async fn handle_tunnels_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        if self.tunnel_edit_visible {
            return self.handle_tunnel_edit_key(key).await;
        }

        let tunnel_count = self.config.tunnels.len();

        match key.code {
            KeyCode::Esc => self.view = View::Connections,
            KeyCode::Char('?') => self.view = View::Help,
            KeyCode::Up | KeyCode::Char('k') => {
                if self.tunnel_selected_index > 0 {
                    self.tunnel_selected_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.tunnel_selected_index + 1 < tunnel_count {
                    self.tunnel_selected_index += 1;
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.tunnel_selected_index = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                if tunnel_count > 0 {
                    self.tunnel_selected_index = tunnel_count - 1;
                }
            }
            KeyCode::Enter => {
                if tunnel_count > 0 {
                    self.open_edit_tunnel();
                } else {
                    self.open_new_tunnel_edit();
                }
            }
            KeyCode::Char('n') => {
                self.open_new_tunnel_edit();
            }
            KeyCode::Char('d') => {
                self.delete_selected_tunnel().await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_tunnel_edit_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        if self.tunnel_editing {
            match key.code {
                KeyCode::Esc => {
                    self.tunnel_editing = false;
                    self.tunnel_temp_buffer.clear();
                }
                KeyCode::Enter => {
                    if let Some(tunnel) = self.tunnel_edit_draft.as_ref() {
                        if let Some(field) =
                            self.tunnel_field_kind(tunnel, self.tunnel_edit_field_index)
                        {
                            let value = self.tunnel_temp_buffer.clone();
                            self.set_tunnel_field_value(field, value);
                        }
                    }
                    self.tunnel_editing = false;
                    self.tunnel_temp_buffer.clear();
                }
                KeyCode::Backspace => {
                    self.tunnel_temp_buffer.pop();
                }
                KeyCode::Char(c) => {
                    self.tunnel_temp_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        }

        let field_count = self
            .tunnel_edit_draft
            .as_ref()
            .map(|t| self.tunnel_field_count(t))
            .unwrap_or(0);

        if self.tunnel_edit_field_index >= field_count {
            self.tunnel_edit_field_index = field_count.saturating_sub(1);
        }

        match key.code {
            KeyCode::Esc => {
                self.close_tunnel_edit().await?;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.tunnel_edit_field_index > 0 {
                    self.tunnel_edit_field_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.tunnel_edit_field_index + 1 < field_count {
                    self.tunnel_edit_field_index += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(tunnel) = self.tunnel_edit_draft.as_ref() {
                    if let Some(field) =
                        self.tunnel_field_kind(tunnel, self.tunnel_edit_field_index)
                    {
                        match field {
                            TunnelFieldKind::Type => {
                                self.cycle_tunnel_type();
                                let new_count = self
                                    .tunnel_edit_draft
                                    .as_ref()
                                    .map(|t| self.tunnel_field_count(t))
                                    .unwrap_or(0);
                                if self.tunnel_edit_field_index >= new_count {
                                    self.tunnel_edit_field_index = new_count.saturating_sub(1);
                                }
                            }
                            TunnelFieldKind::AutoStart => {
                                self.toggle_tunnel_auto_start();
                            }
                            _ => {
                                self.tunnel_temp_buffer = self
                                    .tunnel_field_value(tunnel, field)
                                    .unwrap_or_default();
                                self.tunnel_editing = true;
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn open_new_tunnel_edit(&mut self) {
        let default_tunnel = self.build_default_tunnel();
        self.tunnel_edit_visible = true;
        self.tunnel_edit_is_new = true;
        self.tunnel_edit_draft = Some(default_tunnel.clone());
        self.tunnel_edit_default = Some(default_tunnel);
        self.tunnel_edit_field_index = 0;
        self.tunnel_editing = false;
        self.tunnel_temp_buffer.clear();
    }

    fn open_edit_tunnel(&mut self) {
        if let Some(tunnel) = self.config.tunnels.get(self.tunnel_selected_index).cloned() {
            self.tunnel_edit_visible = true;
            self.tunnel_edit_is_new = false;
            self.tunnel_edit_draft = Some(tunnel);
            self.tunnel_edit_default = None;
            self.tunnel_edit_field_index = 0;
            self.tunnel_editing = false;
            self.tunnel_temp_buffer.clear();
        } else {
            self.status_message = Some("No tunnel selected".to_string());
        }
    }

    async fn close_tunnel_edit(&mut self) -> Result<()> {
        let draft = match self.tunnel_edit_draft.clone() {
            Some(d) => d,
            None => {
                self.tunnel_edit_visible = false;
                return Ok(());
            }
        };

        let name = draft.name().to_string();
        if name.trim().is_empty() {
            self.status_message = Some("Tunnel name is required".to_string());
            return Ok(());
        }

        if self.tunnel_edit_is_new {
            if let Some(default) = self.tunnel_edit_default.as_ref() {
                if &draft == default {
                    self.reset_tunnel_edit_state();
                    return Ok(());
                }
            }

            if self.tunnel_name_exists(&name, None) {
                self.status_message = Some("Tunnel name already exists".to_string());
                return Ok(());
            }

            self.config.tunnels.push(draft);
            self.config.save().await?;
            self.tunnel_selected_index = self.config.tunnels.len().saturating_sub(1);
            self.status_message = Some(format!("Added tunnel: {}", name));
        } else {
            let idx = self.tunnel_selected_index;
            if idx >= self.config.tunnels.len() {
                self.reset_tunnel_edit_state();
                return Ok(());
            }

            if self.tunnel_name_exists(&name, Some(idx)) {
                self.status_message = Some("Tunnel name already exists".to_string());
                return Ok(());
            }

            let old_name = self.config.tunnels[idx].name().to_string();
            self.config.tunnels[idx] = draft;
            if old_name != name {
                self.rename_tunnel_references(&old_name, &name);
            }
            self.config.save().await?;
            self.status_message = Some(format!("Updated tunnel: {}", name));
        }

        self.reset_tunnel_edit_state();
        Ok(())
    }

    async fn delete_selected_tunnel(&mut self) -> Result<()> {
        if self.config.tunnels.is_empty() {
            self.status_message = Some("No tunnel to delete".to_string());
            return Ok(());
        }

        let idx = self.tunnel_selected_index.min(self.config.tunnels.len() - 1);
        let name = self.config.tunnels[idx].name().to_string();
        self.config.tunnels.remove(idx);
        self.remove_tunnel_references(&name);
        self.config.save().await?;

        if self.config.tunnels.is_empty() {
            self.tunnel_selected_index = 0;
        } else if self.tunnel_selected_index >= self.config.tunnels.len()
            && self.tunnel_selected_index > 0
        {
            self.tunnel_selected_index = self.config.tunnels.len() - 1;
        }

        self.status_message = Some(format!("Deleted tunnel: {}", name));
        Ok(())
    }

    fn reset_tunnel_edit_state(&mut self) {
        self.tunnel_edit_visible = false;
        self.tunnel_edit_is_new = false;
        self.tunnel_edit_draft = None;
        self.tunnel_edit_default = None;
        self.tunnel_edit_field_index = 0;
        self.tunnel_editing = false;
        self.tunnel_temp_buffer.clear();
    }

    fn build_default_tunnel(&self) -> crate::config::TunnelConfig {
        let idx = self.config.tunnels.len() + 1;
        crate::config::TunnelConfig::Local {
            name: format!("tunnel-{}", idx),
            bind_addr: "127.0.0.1".to_string(),
            bind_port: 8080,
            remote_host: "localhost".to_string(),
            remote_port: 80,
            auto_start: false,
        }
    }

    fn tunnel_name_exists(&self, name: &str, exclude_idx: Option<usize>) -> bool {
        self.config
            .tunnels
            .iter()
            .enumerate()
            .any(|(idx, t)| t.name() == name && Some(idx) != exclude_idx)
    }

    fn remove_tunnel_references(&mut self, name: &str) {
        for group in &mut self.config.groups {
            for host in &mut group.hosts {
                host.tunnels.retain(|t| t.name() != name);
            }
        }
        for host in &mut self.config.hosts {
            host.tunnels.retain(|t| t.name() != name);
        }
    }

    fn rename_tunnel_references(&mut self, old_name: &str, new_name: &str) {
        for group in &mut self.config.groups {
            for host in &mut group.hosts {
                for tunnel in &mut host.tunnels {
                    if tunnel.name() == old_name {
                        *tunnel = crate::config::TunnelRef::Name(new_name.to_string());
                    }
                }
            }
        }
        for host in &mut self.config.hosts {
            for tunnel in &mut host.tunnels {
                if tunnel.name() == old_name {
                    *tunnel = crate::config::TunnelRef::Name(new_name.to_string());
                }
            }
        }
    }

    fn tunnel_field_count(&self, tunnel: &crate::config::TunnelConfig) -> usize {
        match tunnel {
            crate::config::TunnelConfig::Local { .. } => 7,
            crate::config::TunnelConfig::Remote { .. } => 7,
            crate::config::TunnelConfig::Dynamic { .. } => 5,
        }
    }

    fn tunnel_field_kind(
        &self,
        tunnel: &crate::config::TunnelConfig,
        index: usize,
    ) -> Option<TunnelFieldKind> {
        let fields = match tunnel {
            crate::config::TunnelConfig::Local { .. } => vec![
                TunnelFieldKind::Name,
                TunnelFieldKind::Type,
                TunnelFieldKind::AutoStart,
                TunnelFieldKind::BindAddr,
                TunnelFieldKind::BindPort,
                TunnelFieldKind::RemoteHost,
                TunnelFieldKind::RemotePort,
            ],
            crate::config::TunnelConfig::Remote { .. } => vec![
                TunnelFieldKind::Name,
                TunnelFieldKind::Type,
                TunnelFieldKind::AutoStart,
                TunnelFieldKind::RemoteAddr,
                TunnelFieldKind::RemotePort,
                TunnelFieldKind::LocalHost,
                TunnelFieldKind::LocalPort,
            ],
            crate::config::TunnelConfig::Dynamic { .. } => vec![
                TunnelFieldKind::Name,
                TunnelFieldKind::Type,
                TunnelFieldKind::AutoStart,
                TunnelFieldKind::BindAddr,
                TunnelFieldKind::BindPort,
            ],
        };

        fields.get(index).copied()
    }

    fn tunnel_field_value(
        &self,
        tunnel: &crate::config::TunnelConfig,
        field: TunnelFieldKind,
    ) -> Option<String> {
        match field {
            TunnelFieldKind::Name => Some(tunnel.name().to_string()),
            TunnelFieldKind::Type => Some(tunnel.type_label().to_string()),
            TunnelFieldKind::AutoStart => Some(if tunnel.auto_start() {
                "Yes".to_string()
            } else {
                "No".to_string()
            }),
            TunnelFieldKind::BindAddr => match tunnel {
                crate::config::TunnelConfig::Local { bind_addr, .. } => Some(bind_addr.clone()),
                crate::config::TunnelConfig::Dynamic { bind_addr, .. } => Some(bind_addr.clone()),
                _ => None,
            },
            TunnelFieldKind::BindPort => match tunnel {
                crate::config::TunnelConfig::Local { bind_port, .. } => {
                    Some(bind_port.to_string())
                }
                crate::config::TunnelConfig::Dynamic { bind_port, .. } => {
                    Some(bind_port.to_string())
                }
                _ => None,
            },
            TunnelFieldKind::RemoteHost => match tunnel {
                crate::config::TunnelConfig::Local { remote_host, .. } => {
                    Some(remote_host.clone())
                }
                _ => None,
            },
            TunnelFieldKind::RemotePort => match tunnel {
                crate::config::TunnelConfig::Local { remote_port, .. } => {
                    Some(remote_port.to_string())
                }
                crate::config::TunnelConfig::Remote { remote_port, .. } => {
                    Some(remote_port.to_string())
                }
                _ => None,
            },
            TunnelFieldKind::RemoteAddr => match tunnel {
                crate::config::TunnelConfig::Remote { remote_addr, .. } => {
                    Some(remote_addr.clone())
                }
                _ => None,
            },
            TunnelFieldKind::LocalHost => match tunnel {
                crate::config::TunnelConfig::Remote { local_host, .. } => {
                    Some(local_host.clone())
                }
                _ => None,
            },
            TunnelFieldKind::LocalPort => match tunnel {
                crate::config::TunnelConfig::Remote { local_port, .. } => {
                    Some(local_port.to_string())
                }
                _ => None,
            },
        }
    }

    fn set_tunnel_field_value(&mut self, field: TunnelFieldKind, value: String) {
        let value = value.trim().to_string();
        if let Some(tunnel) = self.tunnel_edit_draft.as_mut() {
            match field {
                TunnelFieldKind::Name => match tunnel {
                    crate::config::TunnelConfig::Local { name, .. } => *name = value,
                    crate::config::TunnelConfig::Remote { name, .. } => *name = value,
                    crate::config::TunnelConfig::Dynamic { name, .. } => *name = value,
                },
                TunnelFieldKind::BindAddr => match tunnel {
                    crate::config::TunnelConfig::Local { bind_addr, .. } => *bind_addr = value,
                    crate::config::TunnelConfig::Dynamic { bind_addr, .. } => *bind_addr = value,
                    _ => {}
                },
                TunnelFieldKind::BindPort => {
                    if let Ok(port) = value.parse::<u16>() {
                        match tunnel {
                            crate::config::TunnelConfig::Local { bind_port, .. } => {
                                *bind_port = port
                            }
                            crate::config::TunnelConfig::Dynamic { bind_port, .. } => {
                                *bind_port = port
                            }
                            _ => {}
                        }
                    }
                }
                TunnelFieldKind::RemoteHost => match tunnel {
                    crate::config::TunnelConfig::Local { remote_host, .. } => {
                        *remote_host = value
                    }
                    _ => {}
                },
                TunnelFieldKind::RemotePort => {
                    if let Ok(port) = value.parse::<u16>() {
                        match tunnel {
                            crate::config::TunnelConfig::Local { remote_port, .. } => {
                                *remote_port = port
                            }
                            crate::config::TunnelConfig::Remote { remote_port, .. } => {
                                *remote_port = port
                            }
                            _ => {}
                        }
                    }
                }
                TunnelFieldKind::RemoteAddr => match tunnel {
                    crate::config::TunnelConfig::Remote { remote_addr, .. } => {
                        *remote_addr = value
                    }
                    _ => {}
                },
                TunnelFieldKind::LocalHost => match tunnel {
                    crate::config::TunnelConfig::Remote { local_host, .. } => {
                        *local_host = value
                    }
                    _ => {}
                },
                TunnelFieldKind::LocalPort => {
                    if let Ok(port) = value.parse::<u16>() {
                        if let crate::config::TunnelConfig::Remote { local_port, .. } = tunnel {
                            *local_port = port;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn cycle_tunnel_type(&mut self) {
        if let Some(tunnel) = self.tunnel_edit_draft.as_mut() {
            let name = tunnel.name().to_string();
            let auto_start = tunnel.auto_start();
            *tunnel = match tunnel {
                crate::config::TunnelConfig::Local { .. } => crate::config::TunnelConfig::Remote {
                    name,
                    remote_addr: "0.0.0.0".to_string(),
                    remote_port: 2222,
                    local_host: "127.0.0.1".to_string(),
                    local_port: 22,
                    auto_start,
                },
                crate::config::TunnelConfig::Remote { .. } => crate::config::TunnelConfig::Dynamic {
                    name,
                    bind_addr: "127.0.0.1".to_string(),
                    bind_port: 1080,
                    auto_start,
                },
                crate::config::TunnelConfig::Dynamic { .. } => crate::config::TunnelConfig::Local {
                    name,
                    bind_addr: "127.0.0.1".to_string(),
                    bind_port: 8080,
                    remote_host: "localhost".to_string(),
                    remote_port: 80,
                    auto_start,
                },
            };
        }
    }

    fn toggle_tunnel_auto_start(&mut self) {
        if let Some(tunnel) = self.tunnel_edit_draft.as_mut() {
            match tunnel {
                crate::config::TunnelConfig::Local { auto_start, .. } => {
                    *auto_start = !*auto_start
                }
                crate::config::TunnelConfig::Remote { auto_start, .. } => {
                    *auto_start = !*auto_start
                }
                crate::config::TunnelConfig::Dynamic { auto_start, .. } => {
                    *auto_start = !*auto_start
                }
            }
        }
    }

    async fn handle_keys_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => self.view = View::Connections,
            KeyCode::Char('?') => self.view = View::Help,
            _ => {}
        }
        Ok(())
    }

    async fn handle_settings_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        // Number of categories and items per category
        // Number of categories and items per category
        // Number of categories and items per category
        const CATEGORIES: &[&str] = &["Appearance", "SSH", "Keys", "Logging", "Keymap", "About"];
        // Items per category: [Appearance, SSH, Keys(dynamic), Logging, Keymap, About]
        let items_per_category: Vec<usize> =
            vec![5, 3, self.key_manager.list_keys().len().max(1), 2, 0, 0];

        // If dropdown is open, handle dropdown navigation
        if self.settings_dropdown_open {
            match key.code {
                KeyCode::Esc => {
                    self.settings_dropdown_open = false;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    // This will be handled in the render - we cycle through options
                    // For now, apply previous option directly
                    self.apply_dropdown_prev().await?;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.apply_dropdown_next().await?;
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    // Confirm and close dropdown
                    self.settings_dropdown_open = false;
                    self.config.save().await?;
                }
                _ => {}
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.view = View::Connections;
                self.settings_category = 0;
                self.settings_item = 0;
            }
            KeyCode::Char('?') => self.view = View::Help,

            // Category navigation (Tab or Left/Right)
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                self.settings_category = (self.settings_category + 1) % CATEGORIES.len();
                self.settings_item = 0;
            }
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
                if self.settings_category == 0 {
                    self.settings_category = CATEGORIES.len() - 1;
                } else {
                    self.settings_category -= 1;
                }
                self.settings_item = 0;
            }

            // Item navigation (Up/Down)
            KeyCode::Up | KeyCode::Char('k') => {
                let max_items = items_per_category[self.settings_category];
                if max_items > 0 {
                    if self.settings_item == 0 {
                        self.settings_item = max_items - 1;
                    } else {
                        self.settings_item -= 1;
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max_items = items_per_category[self.settings_category];
                if max_items > 0 {
                    self.settings_item = (self.settings_item + 1) % max_items;
                }
            }

            // Toggle or open dropdown
            KeyCode::Enter | KeyCode::Char(' ') => {
                match self.settings_category {
                    0 => {
                        // Appearance
                        match self.settings_item {
                            0 => {
                                // Theme - open dropdown
                                self.settings_dropdown_open = true;
                            }
                            1 => {
                                // Mouse enabled - toggle
                                self.config.settings.ui.mouse_enabled =
                                    !self.config.settings.ui.mouse_enabled;
                                self.config.save().await?;
                            }
                            2 => {
                                // Status bar - toggle
                                self.config.settings.ui.show_status_bar =
                                    !self.config.settings.ui.show_status_bar;
                                self.config.save().await?;
                            }
                            3 => {
                                // Scrollback - toggle between presets
                                self.config.settings.ui.scrollback_lines =
                                    match self.config.settings.ui.scrollback_lines {
                                        1000 => 5000,
                                        5000 => 10000,
                                        10000 => 50000,
                                        50000 => 100000,
                                        _ => 1000,
                                    };
                                self.config.save().await?;
                            }
                            4 => {
                                // Graph style - open dropdown
                                self.settings_dropdown_open = true;
                            }
                            _ => {}
                        }
                    }
                    1 => {
                        // SSH
                        match self.settings_item {
                            0 => {
                                // Connection timeout - cycle through presets
                                self.config.settings.ssh.connection_timeout =
                                    match self.config.settings.ssh.connection_timeout {
                                        10 => 30,
                                        30 => 60,
                                        60 => 120,
                                        _ => 10,
                                    };
                                self.config.save().await?;
                            }
                            1 => {
                                // Keepalive - cycle
                                self.config.settings.ssh.keepalive_interval =
                                    match self.config.settings.ssh.keepalive_interval {
                                        0 => 15,
                                        15 => 30,
                                        30 => 60,
                                        _ => 0,
                                    };
                                self.config.save().await?;
                            }
                            2 => {
                                // Reconnect attempts - cycle
                                self.config.settings.ssh.reconnect_attempts =
                                    match self.config.settings.ssh.reconnect_attempts {
                                        0 => 1,
                                        1 => 3,
                                        3 => 5,
                                        5 => 10,
                                        _ => 0,
                                    };
                                self.config.save().await?;
                            }
                            _ => {}
                        }
                    }
                    2 => {
                        // Logging
                        match self.settings_item {
                            0 => {
                                // Logging enabled - toggle
                                self.config.settings.logging.enabled =
                                    !self.config.settings.logging.enabled;
                                self.config.save().await?;
                            }
                            1 => {
                                // Log format - open dropdown
                                self.settings_dropdown_open = true;
                            }
                            _ => {}
                        }
                    }
                    3 => {
                        // Logging
                        match self.settings_item {
                            0 => {
                                // Logging enabled - toggle
                                self.config.settings.logging.enabled =
                                    !self.config.settings.logging.enabled;
                                self.config.save().await?;
                            }
                            1 => {
                                // Log format - open dropdown
                                self.settings_dropdown_open = true;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Char('n') => {
                if self.settings_category == 2 {
                    // Generate new key
                    // For now, auto-generate ED25519
                    let mut path = self.key_manager.ssh_dir().to_path_buf();
                    let name = format!("id_ed25519_{}", chrono::Utc::now().timestamp());
                    path.push(&name);

                    match self.key_manager.generate_ed25519(
                        &path,
                        &format!("{}@rustyssh", whoami::username()),
                        None,
                    ) {
                        Ok(_) => {
                            self.status_message = Some(format!("Generated {}", name));
                            let _ = self.refresh_keys();
                        }
                        Err(e) => {
                            self.status_message = Some(format!("Error: {}", e));
                        }
                    }
                }
            }
            KeyCode::Char('d') => {
                if self.settings_category == 2 {
                    // Delete selected key
                    // Clone needed data to avoid immutable borrow during deletion
                    let key_data = {
                        let keys = self.key_manager.list_keys();
                        if !keys.is_empty() && self.settings_item < keys.len() {
                            let key = keys[self.settings_item];
                            Some((
                                key.path.clone(),
                                key.path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string(),
                            ))
                        } else {
                            None
                        }
                    };

                    if let Some((path, name)) = key_data {
                        match self.key_manager.delete_key(&path) {
                            Ok(_) => {
                                self.status_message = Some(format!("Deleted {}", name));
                                let _ = self.refresh_keys();
                                if self.settings_item > 0 {
                                    self.settings_item -= 1;
                                }
                            }
                            Err(e) => {
                                self.status_message = Some(format!("Error: {}", e));
                            }
                        }
                    }
                }
            }
            KeyCode::Char('r') => {
                if self.settings_category == 2 {
                    let _ = self.refresh_keys();
                    self.status_message = Some("Refreshed".to_string());
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Apply next option in dropdown
    async fn apply_dropdown_next(&mut self) -> Result<()> {
        const THEMES: &[&str] = &["tokyo-night", "gruvbox-dark", "dracula", "nord"];
        const GRAPH_STYLES: &[&str] = &["braille", "block", "ascii"];
        const LOG_FORMATS: &[&str] = &["timestamped", "raw"];

        match self.settings_category {
            0 => match self.settings_item {
                0 => {
                    // Theme
                    let current = THEMES
                        .iter()
                        .position(|&t| t == self.config.settings.ui.theme)
                        .unwrap_or(0);
                    let next = (current + 1) % THEMES.len();
                    self.config.settings.ui.theme = THEMES[next].to_string();
                    self.apply_theme(&THEMES[next].to_string());
                }
                4 => {
                    // Graph style
                    let current = GRAPH_STYLES
                        .iter()
                        .position(|&s| s == self.config.settings.ui.graph_style)
                        .unwrap_or(0);
                    let next = (current + 1) % GRAPH_STYLES.len();
                    self.config.settings.ui.graph_style = GRAPH_STYLES[next].to_string();
                }
                _ => {}
            },
            3 => match self.settings_item {
                1 => {
                    // Log format
                    let current = LOG_FORMATS
                        .iter()
                        .position(|&f| f == self.config.settings.logging.format)
                        .unwrap_or(0);
                    let next = (current + 1) % LOG_FORMATS.len();
                    self.config.settings.logging.format = LOG_FORMATS[next].to_string();
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }

    /// Apply previous option in dropdown
    async fn apply_dropdown_prev(&mut self) -> Result<()> {
        const THEMES: &[&str] = &["tokyo-night", "gruvbox-dark", "dracula", "nord"];
        const GRAPH_STYLES: &[&str] = &["braille", "block", "ascii"];
        const LOG_FORMATS: &[&str] = &["timestamped", "raw"];

        match self.settings_category {
            0 => match self.settings_item {
                0 => {
                    // Theme
                    let current = THEMES
                        .iter()
                        .position(|&t| t == self.config.settings.ui.theme)
                        .unwrap_or(0);
                    let prev = if current == 0 {
                        THEMES.len() - 1
                    } else {
                        current - 1
                    };
                    self.config.settings.ui.theme = THEMES[prev].to_string();
                    self.apply_theme(&THEMES[prev].to_string());
                }
                4 => {
                    // Graph style
                    let current = GRAPH_STYLES
                        .iter()
                        .position(|&s| s == self.config.settings.ui.graph_style)
                        .unwrap_or(0);
                    let prev = if current == 0 {
                        GRAPH_STYLES.len() - 1
                    } else {
                        current - 1
                    };
                    self.config.settings.ui.graph_style = GRAPH_STYLES[prev].to_string();
                }
                _ => {}
            },
            3 => match self.settings_item {
                1 => {
                    // Log format
                    let current = LOG_FORMATS
                        .iter()
                        .position(|&f| f == self.config.settings.logging.format)
                        .unwrap_or(0);
                    let prev = if current == 0 {
                        LOG_FORMATS.len() - 1
                    } else {
                        current - 1
                    };
                    self.config.settings.logging.format = LOG_FORMATS[prev].to_string();
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }

    /// Apply a theme by name
    fn apply_theme(&mut self, theme_name: &str) {
        use crate::tui::{dracula, gruvbox_dark, nord};
        self.theme = match theme_name {
            "gruvbox-dark" => gruvbox_dark(),
            "dracula" => dracula(),
            "nord" => nord(),
            _ => Theme::default(), // tokyo-night
        };
    }

    async fn handle_help_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                self.view = View::Connections;
            }
            _ => {}
        }
        Ok(())
    }

    /// Convert key event to bytes for SSH transmission
    fn key_to_bytes(&self, key: &crossterm::event::KeyEvent) -> Vec<u8> {
        match key.code {
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    // Ctrl+A = 0x01, Ctrl+B = 0x02, etc.
                    let ctrl_code = (c as u8).saturating_sub(b'a' - 1);
                    vec![ctrl_code]
                } else {
                    c.to_string().into_bytes()
                }
            }
            KeyCode::Enter => vec![b'\r'],
            KeyCode::Backspace => {
                if key.modifiers.contains(KeyModifiers::ALT) {
                    // Alt+Backspace = Esc + Backspace (delete word)
                    vec![0x1b, 0x7f]
                } else if key.modifiers.contains(KeyModifiers::CONTROL) {
                    // Ctrl+Backspace = Ctrl+W (delete word backward)
                    vec![0x17]
                } else {
                    vec![0x7f]
                }
            }
            KeyCode::Tab => vec![b'\t'],
            KeyCode::Esc => vec![0x1b],
            KeyCode::Up => vec![0x1b, b'[', b'A'],
            KeyCode::Down => vec![0x1b, b'[', b'B'],
            KeyCode::Right => vec![0x1b, b'[', b'C'],
            KeyCode::Left => vec![0x1b, b'[', b'D'],
            KeyCode::Home => vec![0x1b, b'[', b'H'],
            KeyCode::End => vec![0x1b, b'[', b'F'],
            KeyCode::PageUp => vec![0x1b, b'[', b'5', b'~'],
            KeyCode::PageDown => vec![0x1b, b'[', b'6', b'~'],
            KeyCode::Delete => vec![0x1b, b'[', b'3', b'~'],
            KeyCode::Insert => vec![0x1b, b'[', b'2', b'~'],
            KeyCode::F(n) => match n {
                1 => vec![0x1b, b'O', b'P'],
                2 => vec![0x1b, b'O', b'Q'],
                3 => vec![0x1b, b'O', b'R'],
                4 => vec![0x1b, b'O', b'S'],
                5 => vec![0x1b, b'[', b'1', b'5', b'~'],
                6 => vec![0x1b, b'[', b'1', b'7', b'~'],
                7 => vec![0x1b, b'[', b'1', b'8', b'~'],
                8 => vec![0x1b, b'[', b'1', b'9', b'~'],
                9 => vec![0x1b, b'[', b'2', b'0', b'~'],
                10 => vec![0x1b, b'[', b'2', b'1', b'~'],
                11 => vec![0x1b, b'[', b'2', b'3', b'~'],
                12 => vec![0x1b, b'[', b'2', b'4', b'~'],
                _ => vec![],
            },
            _ => vec![],
        }
    }

    /// Copy selected text to clipboard
    fn copy_selection_to_clipboard(&mut self) {
        // First, get the selected text without holding a mutable borrow
        let text = if let Some(session_id) = self.active_session {
            if let Some(session) = self.sessions.get(session_id) {
                session.get_selected_text()
            } else {
                None
            }
        } else {
            None
        };

        if let Some(text) = text {
            // Try multiple clipboard methods for reliability
            let success = self.set_clipboard_text(&text);
            if success {
                self.status_message = Some(format!("Copied {} chars", text.len()));
                // Now clear selection
                if let Some(session_id) = self.active_session {
                    if let Some(session) = self.sessions.get_mut(session_id) {
                        session.clear_selection();
                    }
                }
            }
        } else {
            self.status_message = Some("No text selected".to_string());
        }
    }

    /// Set clipboard text using multiple methods for reliability
    fn set_clipboard_text(&mut self, text: &str) -> bool {
        // On Linux, detect Wayland vs X11 and use appropriate tools
        #[cfg(target_os = "linux")]
        {
            let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();

            if is_wayland {
                // On Wayland, try wl-copy first
                if Self::set_clipboard_via_command("wl-copy", &[], text) {
                    return true;
                }
            }

            // Try xclip (works on X11, may work on XWayland)
            if Self::set_clipboard_via_command("xclip", &["-selection", "clipboard"], text) {
                // Also set primary selection for middle-click paste
                let _ = Self::set_clipboard_via_command("xclip", &["-selection", "primary"], text);
                return true;
            }

            // Try xsel as fallback
            if Self::set_clipboard_via_command("xsel", &["--clipboard", "--input"], text) {
                let _ = Self::set_clipboard_via_command("xsel", &["--primary", "--input"], text);
                return true;
            }

            // Try wl-copy as fallback (in case WAYLAND_DISPLAY wasn't set but we're on Wayland)
            if !is_wayland && Self::set_clipboard_via_command("wl-copy", &[], text) {
                return true;
            }

            // Fall back to arboard
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                use arboard::{LinuxClipboardKind, SetExtLinux};
                if clipboard
                    .set()
                    .clipboard(LinuxClipboardKind::Clipboard)
                    .text(text.to_string())
                    .is_ok()
                {
                    self.clipboard = Some(clipboard);
                    return true;
                }
            }

            self.status_message = Some("Copy failed: no clipboard tool available".to_string());
            false
        }

        #[cfg(not(target_os = "linux"))]
        {
            match arboard::Clipboard::new() {
                Ok(mut clipboard) => match clipboard.set_text(text) {
                    Ok(_) => {
                        self.clipboard = Some(clipboard);
                        true
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Copy failed: {}", e));
                        false
                    }
                },
                Err(e) => {
                    self.status_message = Some(format!("Clipboard init error: {}", e));
                    false
                }
            }
        }
    }

    /// Set clipboard via external command (Linux)
    #[cfg(target_os = "linux")]
    fn set_clipboard_via_command(cmd: &str, args: &[&str], text: &str) -> bool {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut child = match Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => return false,
        };

        if let Some(mut stdin) = child.stdin.take() {
            if stdin.write_all(text.as_bytes()).is_err() {
                return false;
            }
        }

        child.wait().map(|s| s.success()).unwrap_or(false)
    }

    /// Get clipboard text via external command (Linux)
    #[cfg(target_os = "linux")]
    fn get_clipboard_via_command(cmd: &str, args: &[&str]) -> Option<String> {
        use std::process::{Command, Stdio};

        let output = Command::new(cmd)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;

        if output.status.success() {
            String::from_utf8(output.stdout).ok()
        } else {
            None
        }
    }

    /// Paste from clipboard to terminal
    async fn paste_from_clipboard(&mut self) {
        let mut text_to_paste = None;
        let mut error_msg = None;

        // On Linux, try command-line tools first for better Wayland support
        #[cfg(target_os = "linux")]
        {
            let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();

            if is_wayland {
                // On Wayland, try wl-paste first
                if let Some(text) = Self::get_clipboard_via_command("wl-paste", &["-n"]) {
                    text_to_paste = Some(text);
                }
            }

            // Try xclip if not yet successful
            if text_to_paste.is_none() {
                if let Some(text) =
                    Self::get_clipboard_via_command("xclip", &["-selection", "clipboard", "-o"])
                {
                    text_to_paste = Some(text);
                }
            }

            // Try xsel as fallback
            if text_to_paste.is_none() {
                if let Some(text) =
                    Self::get_clipboard_via_command("xsel", &["--clipboard", "--output"])
                {
                    text_to_paste = Some(text);
                }
            }

            // Try wl-paste as fallback (in case WAYLAND_DISPLAY wasn't set)
            if text_to_paste.is_none() && !is_wayland {
                if let Some(text) = Self::get_clipboard_via_command("wl-paste", &["-n"]) {
                    text_to_paste = Some(text);
                }
            }
        }

        // Try arboard as fallback (or primary on non-Linux)
        if text_to_paste.is_none() {
            // Try existing clipboard first
            if let Some(clipboard) = &mut self.clipboard {
                match clipboard.get_text() {
                    Ok(text) => text_to_paste = Some(text),
                    Err(_) => {
                        // Ignore error, try creating new instance below
                    }
                }
            }

            // If not successful yet, create new instance
            if text_to_paste.is_none() {
                match arboard::Clipboard::new() {
                    Ok(mut clipboard) => {
                        match clipboard.get_text() {
                            Ok(text) => {
                                text_to_paste = Some(text);
                                // Store this clipboard for future reuse
                                self.clipboard = Some(clipboard);
                            }
                            Err(e) => error_msg = Some(format!("Paste failed: {}", e)),
                        }
                    }
                    Err(e) => error_msg = Some(format!("Clipboard error: {}", e)),
                }
            }
        }

        // Handle result
        if let Some(text) = text_to_paste {
            if !text.is_empty() {
                // Send pasted text to the active session using bracketed paste mode
                // This tells the shell to treat the entire paste as a single unit,
                // preventing slow character-by-character processing
                if let Some(session_id) = self.active_session {
                    if let Some(channel) = self.channels.get(&session_id) {
                        // Bracketed paste mode: wrap text in escape sequences
                        // \x1b[200~ = start paste, \x1b[201~ = end paste
                        let mut paste_data = Vec::with_capacity(text.len() + 12);
                        paste_data.extend_from_slice(b"\x1b[200~");
                        paste_data.extend_from_slice(text.as_bytes());
                        paste_data.extend_from_slice(b"\x1b[201~");
                        let _ = channel.input_tx.send(paste_data);
                    }
                }
            }
        } else if let Some(msg) = error_msg {
            self.status_message = Some(msg);
        }
    }

    /// Handle keyboard input in find overlay
    async fn handle_find_overlay_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                // Close find overlay
                self.find_overlay_visible = false;
                self.find_query.clear();
                self.find_matches.clear();
                self.find_match_index = 0;
            }
            KeyCode::Enter | KeyCode::F(3) => {
                // Go to next match
                if !self.find_matches.is_empty() {
                    self.find_match_index = (self.find_match_index + 1) % self.find_matches.len();
                    self.scroll_to_find_match();
                }
            }
            KeyCode::Up => {
                // Go to previous match
                if !self.find_matches.is_empty() {
                    if self.find_match_index == 0 {
                        self.find_match_index = self.find_matches.len() - 1;
                    } else {
                        self.find_match_index -= 1;
                    }
                    self.scroll_to_find_match();
                }
            }
            KeyCode::Down => {
                // Go to next match
                if !self.find_matches.is_empty() {
                    self.find_match_index = (self.find_match_index + 1) % self.find_matches.len();
                    self.scroll_to_find_match();
                }
            }
            KeyCode::Char(c) => {
                self.find_query.push(c);
                self.update_find_matches();
                self.scroll_to_find_match();
            }
            KeyCode::Backspace => {
                self.find_query.pop();
                self.update_find_matches();
                self.scroll_to_find_match();
            }
            _ => {}
        }
        Ok(())
    }

    /// Update find matches based on current query
    /// Searches through visible terminal content
    fn update_find_matches(&mut self) {
        self.find_matches.clear();
        self.find_match_index = 0;

        if self.find_query.is_empty() {
            return;
        }

        if let Some(session_id) = self.active_session {
            if let Some(session) = self.sessions.get(session_id) {
                let screen = session.screen();
                let (_rows, cols) = screen.size();
                let query_lower = self.find_query.to_lowercase();

                // screen.rows(start_col, width) returns iterator over ALL rows
                // where start_col=0 and width=cols gives us full row content
                for (row_idx, line) in screen.rows(0, cols).enumerate() {
                    let line_lower = line.to_lowercase();

                    // Search for query in line (case-insensitive)
                    let mut search_start = 0;
                    while let Some(pos) = line_lower[search_start..].find(&query_lower) {
                        let start_col = (search_start + pos) as u16;
                        let end_col = start_col + self.find_query.len() as u16 - 1;
                        self.find_matches.push((row_idx as u16, start_col, end_col));
                        search_start += pos + 1;
                    }
                }
            }
        }
    }

    /// Highlight current find match
    fn scroll_to_find_match(&mut self) {
        if let Some(&(row, start_col, end_col)) = self.find_matches.get(self.find_match_index) {
            if let Some(session_id) = self.active_session {
                if let Some(session) = self.sessions.get_mut(session_id) {
                    // Select the match for highlighting
                    session.selection = Some(crate::ssh::TextSelection {
                        start: (row, start_col),
                        end: (row, end_col),
                    });
                    session.is_selecting = false;
                }
            }
        }
    }

    /// Handle keyboard input for detail view
    async fn handle_detail_view_key(&mut self, key: crossterm::event::KeyEvent) -> Result<bool> {
        if self.editing_detail {
            match key.code {
                KeyCode::Esc => {
                    self.editing_detail = false;
                    self.temp_edit_buffer.clear();
                }
                KeyCode::Enter => {
                    // Save changes
                    self.save_detail_edit().await?;
                    self.editing_detail = false;
                    self.temp_edit_buffer.clear();
                }
                KeyCode::Backspace => {
                    self.temp_edit_buffer.pop();
                }
                KeyCode::Char(c) => {
                    self.temp_edit_buffer.push(c);
                }
                _ => {}
            }
            return Ok(true);
        }

        let mut handled = true;

        match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                // Return focus to host list
                self.detail_view_focused = false;
                self.editing_detail = false;
            }
            KeyCode::Esc => {
                self.detail_view_focused = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.detail_view_item_index > 0 {
                    self.detail_view_item_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                // Max items: Name, Host, Port, User, Auth, Remember, Proxy, Tunnels
                if self.detail_view_item_index < 7 {
                    self.detail_view_item_index += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.start_detail_edit().await?;
            }
            _ => {
                handled = false;
            }
        }
        Ok(handled)
    }

    /// Start editing the selected detail field
    async fn start_detail_edit(&mut self) -> Result<()> {
        let mut initial_value = None;
        let mut is_toggle = false;
        let mut open_proxy = false;
        let mut open_tunnel_picker = false;

        if let Some(host) = self.current_edit_host() {
            match self.detail_view_item_index {
                0 => initial_value = Some(host.name.clone()),
                1 => initial_value = Some(host.hostname.clone()),
                2 => initial_value = Some(host.port.to_string()),
                3 => initial_value = Some(host.username.clone()),
                4 => is_toggle = true,
                5 => is_toggle = true,
                6 => open_proxy = true,
                7 => open_tunnel_picker = true,
                _ => {}
            }
        }

        if open_proxy {
            self.open_proxy_edit();
            return Ok(());
        }

        if open_tunnel_picker {
            self.open_tunnel_picker();
            return Ok(());
        }

        if is_toggle {
            match self.detail_view_item_index {
                4 => self.toggle_auth_method().await?,
                5 => self.toggle_remember_password().await?,
                _ => {}
            }
        } else if let Some(val) = initial_value {
            self.temp_edit_buffer = val;
            self.editing_detail = true;
        }
        Ok(())
    }

    /// Toggle auth method (simplified for inline edit)
    async fn toggle_auth_method(&mut self) -> Result<()> {
        self.modify_edit_host(|host| {
            use crate::config::AuthMethod;
            match host.auth {
                AuthMethod::Password => {
                    host.auth = AuthMethod::KeyFile {
                        path: std::path::PathBuf::from("id_rsa"),
                        passphrase_required: false,
                    }
                }
                AuthMethod::KeyFile { .. } => host.auth = AuthMethod::Agent,
                AuthMethod::Agent => host.auth = AuthMethod::Password,
                _ => host.auth = AuthMethod::Password,
            }
        });
        if !self.host_edit_is_new {
            self.config.save().await?;
        }
        Ok(())
    }

    /// Toggle remember password
    async fn toggle_remember_password(&mut self) -> Result<()> {
        self.modify_edit_host(|host| {
            host.remember_password = !host.remember_password;
        });
        if !self.host_edit_is_new {
            self.config.save().await?;
        }
        Ok(())
    }

    /// Save the edited text value
    async fn save_detail_edit(&mut self) -> Result<()> {
        let val = self.temp_edit_buffer.clone();
        let idx = self.detail_view_item_index;

        self.modify_edit_host(|host| match idx {
            0 => host.name = val,
            1 => host.hostname = val,
            2 => {
                if let Ok(p) = val.parse::<u16>() {
                    host.port = p;
                }
            }
            3 => host.username = val,
            _ => {}
        });

        if !self.host_edit_is_new {
            self.config.save().await?;
        }
        Ok(())
    }

    fn current_edit_host(&self) -> Option<&HostConfig> {
        if self.host_edit_is_new {
            self.host_edit_draft.as_ref()
        } else {
            self.selected_host()
        }
    }

    fn modify_edit_host<F>(&mut self, f: F)
    where
        F: FnOnce(&mut HostConfig),
    {
        if self.host_edit_is_new {
            if let Some(host) = self.host_edit_draft.as_mut() {
                f(host);
            }
        } else {
            self.modify_selected_host(f);
        }
    }

    /// Helper to modify the currently selected host
    fn modify_selected_host<F>(&mut self, f: F)
    where
        F: FnOnce(&mut crate::config::HostConfig),
    {
        let mut idx = 0;
        let selected_idx = self.selected_host_index;

        for group in &mut self.config.groups {
            if group.expanded {
                if selected_idx >= idx && selected_idx < idx + group.hosts.len() {
                    let host_idx = selected_idx - idx;
                    if let Some(host) = group.hosts.get_mut(host_idx) {
                        f(host);
                    }
                    return;
                }
                idx += group.hosts.len();
            }
        }

        let ungrouped_idx = selected_idx.saturating_sub(idx);
        if ungrouped_idx < self.config.hosts.len() {
            if let Some(host) = self.config.hosts.get_mut(ungrouped_idx) {
                f(host);
            }
        }
    }
}
