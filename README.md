# 🔐 RustySSH

**A modern, high-performance TUI SSH connection manager built in Rust**

![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)
![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey.svg)

RustySSH is a terminal-based SSH connection manager with a beautiful, btop-inspired interface. Manage your SSH connections, tunnels, and keys all from one elegant TUI application.

## ✨ Features

- **🖥️ Beautiful TUI** - Dark theme with Tokyo Night colors, responsive layout
- **🔗 Connection Management** - Organize hosts in groups, quick connect with Enter
- **🔑 Multiple Auth Methods** - Password, key file, SSH agent, certificates
- **📁 SFTP Browser** - Dual-pane file manager (coming soon)
- **🔀 SSH Tunnels** - Local, remote, and dynamic port forwarding (coming soon)
- **🗝️ Key Management** - View, generate, and manage SSH keys (coming soon)
- **⚡ Fast & Lightweight** - Built with Rust for speed and reliability
- **🎨 Font Detection** - Auto-detects Nerd Fonts, falls back to Unicode/ASCII

## 📦 Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/yourusername/rustyssh.git
cd rustyssh

# Build and install
cargo build --release

# Run
./target/release/rustyssh
```

### Requirements

- Rust 1.70+
- libssh2 (for SSH functionality)
- A terminal with Unicode support

**macOS:**
```bash
brew install libssh2
```

**Ubuntu/Debian:**
```bash
sudo apt install libssh2-1-dev
```

## 🚀 Quick Start

1. **Run RustySSH:**
   ```bash
   cargo run
   ```

2. **Add your first host:**
   - Press `n` to create a new host
   - Press `e` to edit the config in your `$EDITOR`
   - Modify the host settings in YAML format

3. **Connect:**
   - Use `j`/`k` or `↑`/`↓` to select a host
   - Press `Enter` to connect

## ⌨️ Keyboard Shortcuts

### Connections View
| Key | Action |
|-----|--------|
| `Enter` | Connect to selected host |
| `n` | Add new host |
| `e` | Edit config file |
| `d` | Delete selected host |
| `j`/`↓` | Move down |
| `k`/`↑` | Move up |
| `g` | Go to top |
| `G` | Go to bottom |
| `t` | Tunnels view |
| `f` | SFTP view |
| `K` (Shift) | Keys view |
| `s` | Settings |
| `?` | Help |
| `q` | Quit |

### Session View
| Key | Action |
|-----|--------|
| `Shift+Esc` | Return to connections |
| `Ctrl+C` | Disconnect |

### Global
| Key | Action |
|-----|--------|
| `Ctrl+Q` | Quit application |
| `Esc` | Go back / Cancel |

## ⚙️ Configuration

Configuration is stored in `~/.config/rustyssh/config.yaml`:

```yaml
settings:
  ui:
    theme: tokyo_night
    mouse_enabled: true
  ssh:
    connection_timeout: 30
    keepalive_interval: 60

groups:
  - name: Production
    expanded: true
    hosts:
      - name: web-server-1
        hostname: 192.168.1.100
        port: 22
        username: admin
        auth: !Agent
        tags: [web, prod]

  - name: Development
    expanded: true
    hosts:
      - name: dev-box
        hostname: dev.example.com
        username: developer
        auth: !KeyFile
          path: ~/.ssh/id_ed25519
          passphrase_required: false

hosts:
  - name: personal-server
    hostname: my.server.com
    username: user
    auth: !Password
```

### Authentication Methods

```yaml
# SSH Agent (recommended)
auth: !Agent

# Password (prompted on connect)
auth: !Password

# Key File
auth: !KeyFile
  path: ~/.ssh/id_rsa
  passphrase_required: true

# Certificate
auth: !Certificate
  cert_path: ~/.ssh/id_ed25519-cert.pub
  key_path: ~/.ssh/id_ed25519
```

## 🎨 Icon Support

RustySSH auto-detects Nerd Font support and uses appropriate icons:

| Environment | Icons Used |
|-------------|-----------|
| Kitty, Alacritty, WezTerm | Nerd Font glyphs |
| iTerm2, Terminal.app | Unicode/ASCII symbols |

**Force Nerd Fonts:**
```bash
export NERD_FONT=1
rustyssh
```

**Force ASCII:**
```bash
export NERD_FONT=0
rustyssh
```

## 🏗️ Architecture

```
src/
├── main.rs          # Entry point
├── lib.rs           # Library exports
├── app/             # Application state & events
│   ├── events.rs    # Event handling system
│   └── state.rs     # App state machine
├── config/          # Configuration management
│   ├── hosts.rs     # Host & auth definitions
│   └── settings.rs  # App settings
├── ssh/             # SSH functionality
│   ├── auth.rs      # Authentication handlers
│   ├── connection.rs # Connection pool
│   ├── keys.rs      # Key management
│   ├── session.rs   # Terminal emulation
│   └── tunnel.rs    # Port forwarding
├── sftp/            # SFTP functionality
│   ├── browser.rs   # File browser
│   └── transfer.rs  # Transfer queue
├── tui/             # Terminal UI
│   ├── theme.rs     # Color themes
│   ├── icons.rs     # Icon detection
│   ├── ui.rs        # Main renderer
│   ├── views/       # UI views
│   └── widgets/     # Reusable widgets
└── utils/           # Utilities
    └── terminal.rs  # Terminal helpers
```

## 🧰 Tech Stack

- **[Ratatui](https://github.com/ratatui-org/ratatui)** - TUI framework
- **[Crossterm](https://github.com/crossterm-rs/crossterm)** - Terminal manipulation
- **[ssh2](https://github.com/alexcrichton/ssh2-rs)** - SSH2 protocol (libssh2)
- **[vt100](https://github.com/doy/vt100-rust)** - Terminal emulation
- **[Tokio](https://tokio.rs/)** - Async runtime
- **[Serde](https://serde.rs/)** - Serialization

## 🛣️ Roadmap

- [x] Core TUI with views
- [x] Configuration management
- [x] SSH connection integration
- [x] Dynamic icon support
- [ ] SFTP file operations
- [ ] Tunnel management UI
- [ ] Key generation UI
- [ ] Clipboard support
- [ ] Session tabs
- [ ] Search/filter hosts
- [ ] Import from ~/.ssh/config

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Inspired by [btop](https://github.com/aristocratos/btop) for the beautiful TUI aesthetics
- [Tokyo Night](https://github.com/enkia/tokyo-night-vscode-theme) color scheme
