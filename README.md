# 🔐 RustySSH

**A modern, high-performance TUI SSH connection manager built in Rust**

![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)
![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey.svg)

RustySSH is a terminal-based SSH connection manager with a beautiful interface. Manage your SSH connections, tunnels, and keys all from one elegant TUI application.

> [!IMPORTANT]
> This project is **totally vibecoding**. Every line of code is driven by LLM.

## 🧪 Demo

<p align="center">
  <img src="images/demo.gif" alt="RustySSH demo" style="max-width: 980px; width: 100%; height: auto;" />
</p>


## ✨ Features

- **🖥️ Beautiful TUI** - Dark theme with Tokyo Night colors, responsive layout
- **🔗 Connection Management** - Organize hosts in groups, quick connect
- **🔑 Multiple Auth Methods** - Password, key file, SSH agent, certificates
- **🔀 Proxy Support** - Jump hosts, SOCKS4/5, HTTP CONNECT, ProxyCommand
- **📁 SFTP Browser** - Dual-pane file manager
- **💻 Multi-Session** - Multiple concurrent SSH sessions with quick switching
- **🔒 Credential Storage** - Encrypted password vault with master password
- **⚡ Fast & Lightweight** - Built with Rust for speed and reliability

📋 **[Full feature list and roadmap →](docs/feature_plan.md)**

## 📦 Installation

### Requirements

- Rust 1.70+
- libssh2 (for SSH functionality)
- A terminal with Unicode support

### From Source

```bash
git clone https://github.com/yourusername/rustyssh.git
cd rustyssh
cargo build --release
./target/release/rustyssh
```

## 🚀 Quick Start

1. **Run RustySSH:**
   ```bash
   cargo run
   ```

2. **Add your first host:**
   - Press `n` to create a new host
   - Press `e` to edit the config or `E` to edit in your `$EDITOR`

3. **Connect:**
   - Use `j`/`k` or `↑`/`↓` to navigate the hosts
   - Press `Enter` to connect
   - Input your master password and host connection password when you are prompted

## ⌨️ Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Enter` | Connect to selected host |
| `n` | Add new host |
| `e` | Edit config file |
| `d` | Delete selected host |
| `j`/`k` or `↑`/`↓` | Navigate |
| `Tab` | Switch sessions |
| `alt + f` | SFTP view |
| `?` | Help |
| `ctrl + q` | Quit |

## ⚙️ Configuration

Configuration is stored in `~/.config/rustyssh/config.yaml`:

```yaml
settings:
  ui:
    theme: tokyo_night
    mouse_enabled: true
  ssh:
    connection_timeout: 30

groups:
  - name: Production
    hosts:
      - name: web-server
        hostname: 192.168.1.100
        username: admin
        auth: !Agent

hosts:
  - name: personal-server
    hostname: my.server.com
    username: user
    auth: !Password
```

### Authentication Methods

```yaml
auth: !Agent                    # SSH Agent (recommended)
auth: !Password                 # Password (prompted)
auth: !KeyFile                  # Key file
  path: ~/.ssh/id_rsa
auth: !Certificate              # Certificate
  cert_path: ~/.ssh/id-cert.pub
  key_path: ~/.ssh/id_ed25519
```

### Proxy Configuration

```yaml
proxy: !JumpHost               # SSH jump host
  host: bastion-uuid-or-name
proxy: !Socks5                 # SOCKS5 proxy
  address: 127.0.0.1
  port: 1080
  username: proxyuser          # optional
  password: proxypass          # optional
proxy: !Socks4                 # SOCKS4 proxy
  address: 127.0.0.1
  port: 1080
  user_id: socks4-user         # optional
proxy: !Http                   # HTTP CONNECT proxy
  address: proxy.example.com
  port: 8080
  username: proxyuser          # optional
  password: proxypass          # optional
proxy: !ProxyCommand           # Custom command
  command: "nc -x localhost:1080 %h %p"
```

### Tunnel Configuration

```yaml
tunnels:
  - type: local
    name: db-forward
    bind_addr: 127.0.0.1
    bind_port: 3306
    remote_host: db.prod
    remote_port: 3306
    auto_start: true
  - type: dynamic
    name: socks
    bind_addr: 127.0.0.1
    bind_port: 1080

hosts:
  - name: app-server
    hostname: 10.0.0.10
    username: ubuntu
    tunnels: [db-forward, socks]
```

## 📚 Documentation

| Document | Description |
|----------|-------------|
| [Architecture](docs/architecture.md) | System design, module structure, data flow |
| [Source Guide](docs/source.md) | Complete source code reference |
| [Feature Plan](docs/feature_plan.md) | Current features and roadmap |

## 🧰 Tech Stack

- **[Ratatui](https://github.com/ratatui-org/ratatui)** - TUI framework
- **[ssh2](https://github.com/alexcrichton/ssh2-rs)** - SSH2 protocol (libssh2)
- **[vt100](https://github.com/doy/vt100-rust)** - Terminal emulation
- **[Tokio](https://tokio.rs/)** - Async runtime

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

See [Feature Plan](docs/feature_plan.md#contributing) for areas where help is needed.

## 📄 License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [Ratatui](https://ratatui.rs/) for awesome TUI 
- [libssh2](https://github.com/libssh2/libssh2.git) for ssh protocol
