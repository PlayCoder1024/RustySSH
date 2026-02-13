# RustySSH Source Code Guide

Complete source code reference for the RustySSH project.

## Directory Structure

```
src/
├── main.rs              # Entry point, logging setup
├── lib.rs               # Library exports
├── app/                 # Application core
│   ├── mod.rs
│   ├── events.rs        # Event types and handling
│   └── state.rs         # Main App struct (~2500 lines)
├── config/              # Configuration management
│   ├── mod.rs           # Config loading/saving
│   ├── hosts.rs         # Host, auth, proxy definitions
│   └── settings.rs      # UI, SSH, logging settings
├── credentials/         # Secure password storage
│   ├── mod.rs           # CredentialManager
│   ├── master.rs        # Master password (Argon2id)
│   └── vault.rs         # Encrypted credential vault
├── sftp/                # SFTP functionality
│   ├── mod.rs
│   ├── browser.rs       # FilePane, FileBrowser
│   ├── sftp_session.rs  # SFTP session wrapper
│   └── transfer.rs      # Transfer queue
├── ssh/                 # SSH core
│   ├── mod.rs
│   ├── auth.rs          # Authentication handlers
│   ├── connection.rs    # SshConnection, proxies (~900 lines)
│   ├── keys.rs          # SSH key management
│   ├── session.rs       # Terminal session + VT100
│   └── tunnel.rs        # Port forwarding
├── tui/                 # Terminal UI
│   ├── mod.rs
│   ├── ui.rs            # Main render entry
│   ├── theme.rs         # Tokyo Night theme (~350 lines)
│   ├── icons.rs         # Nerd Font detection
│   ├── highlight.rs     # Terminal keyword highlighting
│   ├── terminal_render.rs # VT100 to ratatui conversion
│   ├── views/           # UI views
│   │   ├── mod.rs
│   │   ├── connections.rs
│   │   ├── session.rs
│   │   ├── session_list.rs
│   │   ├── sftp.rs
│   │   ├── tunnels.rs
│   │   ├── keys.rs
│   │   ├── settings.rs
│   │   └── help.rs
│   └── widgets/         # Reusable widgets
│       ├── mod.rs
│       ├── host_list.rs
│       ├── status_bar.rs
│       └── find_overlay.rs
└── utils/               # Utilities
    ├── mod.rs
    └── terminal.rs      # Terminal helpers
```

## Core Files Reference

### `src/main.rs` (21 lines)

Entry point with async main function:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging with env filter
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    // Run the application
    let mut app = App::new().await?;
    app.run().await?;
    Ok(())
}
```

### `src/lib.rs` (19 lines)

Public module exports:

```rust
pub mod app;
pub mod config;
pub mod credentials;
pub mod sftp;
pub mod ssh;
pub mod tui;
pub mod utils;

pub use app::App;
pub use config::Config;
```

---

## Application Layer

### `src/app/state.rs` (~2500 lines)

The heart of the application. Key components:

**Enums:**
- `View` - Current UI view (Connections, Session, SFTP, etc.)
- `AppState` - Running or Quit

**Structs:**
- `SessionInfo` - Session rendering info (avoids borrow conflicts)
- `RenderState` - Complete render state snapshot
- `FilePaneSnapshot`, `FileEntrySnapshot` - SFTP rendering data
- `ActiveChannel` - SSH channel for a session
- `App` - Main application struct

**Key `App` Methods:**

| Method | Purpose |
|--------|---------|
| `new()` | Initialize app, load config |
| `run()` | Main event loop |
| `handle_event()` | Route events to handlers |
| `handle_key()` | Dispatch key events by view |
| `handle_mouse()` | Mouse events (scroll, click, drag select) |
| `connect_to_host()` | Establish SSH connection with proxy support |
| `edit_config()` | Open config in $EDITOR |
| `push_view()` / `pop_view()` | View navigation stack |

**Connection Flow in `connect_to_host()`:**
1. Check for pending connection task
2. Resolve proxy configuration (recursively for jump hosts)
3. Prompt for passwords (with credential manager integration)
4. Spawn async connection task
5. Handle connection result via channel

### `src/app/events.rs` (~120 lines)

Event types and handler trait:

```rust
pub enum AppEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Tick,
    SshData { session_id: Uuid, data: Vec<u8> },
    ConnectionResult { ... },
}

pub trait EventHandler {
    fn handle_event(&mut self, event: AppEvent) -> Result<()>;
}
```

---

## Configuration Layer

### `src/config/mod.rs` (~160 lines)

Main configuration struct:

```rust
pub struct Config {
    pub settings: Settings,
    pub tunnels: Vec<TunnelConfig>,
    pub groups: Vec<HostGroup>,
    pub hosts: Vec<HostConfig>,
}
```

**Methods:**
- `load()` - Load from `~/.config/rustyssh/config.yaml`
- `save()` - Persist to disk
- `config_path()` - Get config file path
- `find_host_by_id()` / `find_host_by_name()` / `find_host_by_hostname()`

### `src/config/hosts.rs` (~277 lines)

**Key Types:**

```rust
// Jump host reference (flexible resolution)
pub enum JumpHostRef {
    ByUuid(Uuid),
    ByHostname(String),
    ByName(String),
}

// Proxy configuration
pub enum ProxyConfig {
    JumpHost { host: JumpHostRef },
    Socks5 { address, port, username?, password? },
    Socks4 { address, port, user_id? },
    Http { address, port, username?, password? },
    ProxyCommand { command: String },
}

// Host configuration
pub struct HostConfig {
    pub id: Uuid,
    pub name: String,
    pub hostname: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthMethod,
    pub proxy: Option<ProxyConfig>,
    pub tags: Vec<String>,
    pub tunnels: Vec<TunnelRef>,
    pub startup_commands: Vec<String>,
    pub environment: HashMap<String, String>,
    pub notes: String,
    pub color: Option<String>,
    pub remember_password: bool,
}

// Authentication methods
pub enum AuthMethod {
    Password,
    KeyFile { path: PathBuf, passphrase_required: bool },
    Agent,
    Certificate { cert_path: PathBuf, key_path: PathBuf },
}

// Tunnel types
pub enum TunnelConfig {
    Local { name, bind_addr, bind_port, remote_host, remote_port, auto_start },
    Remote { name, remote_addr, remote_port, local_host, local_port, auto_start },
    Dynamic { name, bind_addr, bind_port, auto_start },
}
```

### `src/config/settings.rs` (~176 lines)

Application settings:

```rust
pub struct Settings {
    pub ui: UiSettings,
    pub ssh: SshSettings,
    pub logging: LogSettings,
}

pub struct UiSettings {
    pub theme: String,              // Default: "tokyo-night"
    pub mouse_enabled: bool,        // Default: true
    pub show_status_bar: bool,      // Default: true
    pub scrollback_lines: usize,    // Default: 10000
    pub graph_style: String,        // Default: "braille"
    pub terminal_highlight: TerminalHighlightConfig,
}

pub struct SshSettings {
    pub known_hosts_path: PathBuf,
    pub connection_timeout: u32,    // Default: 30
    pub keepalive_interval: u32,    // Default: 30
    pub reconnect_attempts: u32,    // Default: 3
    pub auth_order: Vec<String>,    // Default: [agent, publickey, password]
}
```

---

## SSH Layer

### `src/ssh/connection.rs` (~900 lines)

Core SSH connection handling.

**Proxy Types:**

```rust
pub enum ProxyConnection {
    Direct,
    JumpHost { tunnel_session: Arc<Session>, tunnel_channel: Arc<Mutex<Channel>> },
    Socks5 { address, port, auth: Option<(String, String)> },
    Socks4 { address, port, user_id: Option<String> },
    Http { address, port, auth: Option<(String, String)> },
    ProxyCommand { command, target_host, target_port },
}
```

**`SshConnection` Methods:**

| Method | Purpose |
|--------|---------|
| `connect()` | Direct connection |
| `connect_via_proxy()` | Connection through proxy (main entry) |
| `create_socket_pair()` | Create socket pair for proxying |
| `proxy_channel_to_stream()` | Proxy data channel↔stream |
| `proxy_process_to_stream()` | Proxy process stdin/stdout↔stream |
| `socks5_handshake()` | RFC 1928 SOCKS5 |
| `socks4_handshake()` | SOCKS4/4a |
| `http_connect_handshake()` | HTTP CONNECT |
| `open_shell()` | Open interactive shell |
| `open_direct_tcpip()` | Port forwarding channel |
| `open_sftp()` | SFTP subsystem |
| `exec()` | Execute single command |

**`ConnectionPool`:**
- Manages multiple connections by UUID
- Methods: `add()`, `get()`, `remove()`, `list()`, `count()`

### `src/ssh/session.rs` (~345 lines)

Terminal session with VT100 emulation.

**`Session` Struct:**
```rust
pub struct Session {
    pub id: Uuid,
    pub host_id: Uuid,
    pub name: String,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    parser: Parser,             // VT100 parser
    scroll_offset: usize,       // Scrollback offset
    selection: Option<TextSelection>,
    selecting: bool,
}
```

**Key Methods:**

| Method | Purpose |
|--------|---------|
| `new()` | Create with dimensions |
| `process_data()` | Feed data through VT100 parser |
| `screen()` | Get current screen |
| `screen_lines()` | Get screen as strings |
| `resize()` | Handle terminal resize |
| `scroll_up()` / `scroll_down()` | Scrollback navigation |
| `start_selection()` / `update_selection()` | Text selection |
| `get_selected_text()` | Copy selection |
| `get_all_content_for_search()` | Full buffer for search |

**`SessionManager`:**
- Manages multiple sessions
- Tracks active session
- Methods: `add()`, `get()`, `switch()`, `close()`, `cycle_next()`, `cycle_prev()`

### `src/ssh/tunnel.rs` (~110 lines)

Port forwarding configuration and status tracking.

### `src/ssh/keys.rs` (~230 lines)

SSH key management utilities.

### `src/ssh/auth.rs` (~100 lines)

Authentication helper functions.

---

## SFTP Layer

### `src/sftp/browser.rs` (~480 lines)

Dual-pane file browser.

**`FileEntry` Struct:**
```rust
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<DateTime<Utc>>,
    pub permissions: Option<u32>,
}
```

**`FilePane` Struct:**
```rust
pub struct FilePane {
    pub path: PathBuf,
    pub entries: Vec<FileEntry>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub selected: HashSet<usize>,
    pub is_remote: bool,
    pub sort_order: SortOrder,
    pub filter: String,
    pub show_hidden: bool,
}
```

**Methods:** Navigation, selection, sorting, local/remote loading.

**`FileBrowser`:**
- Contains `left` and `right` `FilePane`
- Tracks `active_pane: PaneSide`

### `src/sftp/sftp_session.rs` (~200 lines)

SFTP session wrapper.

### `src/sftp/transfer.rs` (~230 lines)

File transfer queue with progress tracking.

---

## Credentials Layer

### `src/credentials/mod.rs` (~130 lines)

Main credential interface.

**`CredentialManager`:**

| Method | Purpose |
|--------|---------|
| `new()` | Initialize |
| `has_master_password()` | Check if master set |
| `is_unlocked()` | Check unlock status |
| `setup_master_password()` | First-time setup |
| `unlock()` | Unlock with password |
| `lock()` | Clear encryption key |
| `save_password()` | Store encrypted |
| `get_password()` | Retrieve decrypted |
| `has_saved_password()` | Check existence |
| `delete_password()` | Remove stored |

### `src/credentials/master.rs` (~250 lines)

Master password handling with Argon2id.

### `src/credentials/vault.rs` (~150 lines)

AES-256-GCM encrypted credential storage.

---

## TUI Layer

### `src/tui/theme.rs` (~348 lines)

Tokyo Night inspired theme.

**`Theme` Struct:**
- Color palette (backgrounds, foregrounds, accents, borders)
- Predefined styles: `text()`, `selected()`, `error()`, `success()`, etc.
- Parses hex colors to ratatui `Color`

### `src/tui/icons.rs` (~120 lines)

Icon detection:
- Detects Nerd Font support via terminal environment
- Provides fallback Unicode/ASCII icons
- `get_icon()`, `is_nerd_font_available()`

### `src/tui/terminal_render.rs` (~290 lines)

Converts VT100 screen to ratatui widgets:
- Maps VT100 colors to ratatui colors
- Handles text attributes (bold, italic, underline, etc.)
- Selection highlighting

### `src/tui/highlight.rs` (~370 lines)

Terminal keyword highlighting:
- Configurable patterns for errors, warnings, success, paths, URLs
- Regex-based matching
- Style application

### `src/tui/ui.rs` (~210 lines)

Main render entry point:
- Routes to appropriate view renderer
- Handles overlays (session list, find)

---

## Views

### `src/tui/views/connections.rs` (~450 lines)

Host list with expandable groups:
- Group/host tree rendering
- Selection highlighting
- Status indicators

### `src/tui/views/session.rs` (~230 lines)

Active terminal session:
- Terminal content rendering
- Cursor display
- Scrollback indicator

### `src/tui/views/session_list.rs` (~240 lines)

Multi-session switcher overlay:
- Session list popup
- Quick switching

### `src/tui/views/sftp.rs` (~360 lines)

Dual-pane file browser:
- Left/right pane rendering
- File list with icons
- Transfer queue display

### `src/tui/views/tunnels.rs` (~170 lines)

Tunnel management interface.

### `src/tui/views/keys.rs` (~200 lines)

SSH key viewer.

### `src/tui/views/settings.rs` (~190 lines)

Settings editor interface.

### `src/tui/views/help.rs` (~175 lines)

Keyboard shortcuts reference.

---

## Widgets

### `src/tui/widgets/host_list.rs` (~95 lines)

Reusable host entry widget.

### `src/tui/widgets/status_bar.rs` (~50 lines)

Bottom status bar.

### `src/tui/widgets/find_overlay.rs` (~110 lines)

Terminal text search:
- Search input
- Match count display
- Navigation hints

---

## Tests

```
tests/
├── main.rs              # Test harness
├── common/              # Test helpers
│   ├── mod.rs
│   └── docker_helper.rs
├── docker/              # Docker test infrastructure
│   ├── Dockerfile.openssh
│   └── docker-compose.yml
└── integration/         # Integration tests
    ├── mod.rs
    ├── connection_tests.rs
    ├── config_tests.rs
    └── sftp_tests.rs
```

Integration tests use Docker-based OpenSSH servers.
