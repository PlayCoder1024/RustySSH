# RustySSH Feature Plan

Current implementation status and future roadmap for RustySSH.

## Current Features (Implemented)

### ✅ Core TUI Framework

| Feature | Status | Notes |
|---------|--------|-------|
| Ratatui-based UI | ✅ Complete | Beautiful btop-inspired interface |
| Tokyo Night theme | ✅ Complete | Dark theme with customizable colors |
| Mouse support | ✅ Complete | Click, scroll, drag selection |
| Keyboard navigation | ✅ Complete | Vim-style j/k, arrow keys, shortcuts |
| View navigation | ✅ Complete | Stack-based view history |
| Status bar | ✅ Complete | Context-aware shortcuts display |
| Nerd Font detection | ✅ Complete | Auto-fallback to Unicode/ASCII |

### ✅ Connection Management

| Feature | Status | Notes |
|---------|--------|-------|
| Host list with groups | ✅ Complete | Collapsible groups in YAML config |
| Quick add/edit/delete | ✅ Complete | `n`/`e`/`d` shortcuts |
| Multiple auth methods | ✅ Complete | Password, key file, agent, certificate |
| Connection timeout handling | ✅ Complete | User feedback with "Connecting..." overlay |
| Connection cancellation | ✅ Complete | Esc to cancel pending connection |

### ✅ Proxy Support

| Feature | Status | Notes |
|---------|--------|-------|
| SSH Jump Host | ✅ Complete | ProxyJump-style tunneling |
| SOCKS5 proxy | ✅ Complete | With optional auth |
| SOCKS4 proxy | ✅ Complete | SOCKS4a domain support |
| HTTP CONNECT proxy | ✅ Complete | With optional Basic auth |
| ProxyCommand | ✅ Complete | Custom command execution |
| Proxy chaining | ✅ Complete | Recursive jump host resolution |

### ✅ SSH Session

| Feature | Status | Notes |
|---------|--------|-------|
| VT100 terminal emulation | ✅ Complete | Full vt100 parser integration |
| Scrollback buffer | ✅ Complete | Configurable size (default 10000) |
| Text selection | ✅ Complete | Mouse drag, copy to clipboard |
| Terminal resize | ✅ Complete | Dynamic PTY resize |
| ANSI color support | ✅ Complete | Full 256-color and true color |
| Find in terminal | ✅ Complete | Ctrl+F search with navigation |

### ✅ Multi-Session Support

| Feature | Status | Notes |
|---------|--------|-------|
| Multiple sessions | ✅ Complete | Connect without exiting current |
| Session list overlay | ✅ Complete | Quick switcher UI |
| Session switching | ✅ Complete | Keyboard shortcuts |
| Quick session cycling | ✅ Complete | Tab/Shift+Tab to cycle |

### ✅ SFTP Browser

| Feature | Status | Notes |
|---------|--------|-------|
| Dual-pane file browser | ✅ Complete | Local ↔ Remote |
| Directory navigation | ✅ Complete | Enter to open, Backspace for parent |
| File listing | ✅ Complete | Name, size, permissions, date |
| Sorting options | ✅ Complete | Name, size, modified date |
| Hidden files toggle | ✅ Complete | Show/hide dotfiles |
| Filter/search | ✅ Complete | Filter file list |

### ✅ Credential Management

| Feature | Status | Notes |
|---------|--------|-------|
| Master password | ✅ Complete | Argon2id hashing |
| Encrypted password storage | ✅ Complete | AES-256-GCM |
| OS keyring integration | ✅ Complete | Cross-platform |
| Remember password option | ✅ Complete | Per-host setting |
| Secure memory wiping | ✅ Complete | Zeroize on lock |

### ✅ Configuration

| Feature | Status | Notes |
|---------|--------|-------|
| YAML config file | ✅ Complete | `~/.config/rustyssh/config.yaml` |
| External editor support | ✅ Complete | Opens in $EDITOR |
| UI settings | ✅ Complete | Theme, mouse, scrollback |
| SSH settings | ✅ Complete | Timeout, keepalive, auth order |
| Logging settings | ✅ Complete | Directory, format |
| Terminal highlighting | ✅ Complete | Configurable patterns |

---

## In Progress

### 🔄 SFTP File Operations

| Feature | Status | Target |
|---------|--------|--------|
| Upload files | 🔄 In Progress | v0.2.0 |
| Download files | 🔄 In Progress | v0.2.0 |
| Transfer queue | 🔄 Partial | UI implemented, backend WIP |
| Progress tracking | 🔄 Partial | Display implemented |
| Bulk operations | 📋 Planned | Multi-select ready |

---

## Planned Features

### 📋 Tunnel Management UI (v0.3.0)

| Feature | Priority | Description |
|---------|----------|-------------|
| Local forwarding | High | -L style port forwarding |
| Remote forwarding | High | -R style port forwarding |
| Dynamic SOCKS proxy | High | -D style SOCKS proxy |
| Tunnel status display | Medium | Active/inactive indicators |
| Auto-start tunnels | Medium | Connect-time tunnel setup |
| Tunnel persistence | Low | Keep tunnels across reconnects |

### 📋 Key Management UI (v0.3.0)

| Feature | Priority | Description |
|---------|----------|-------------|
| View existing keys | High | List ~/.ssh/ keys |
| Key details display | High | Type, fingerprint, comment |
| Key generation | Medium | Ed25519, RSA, ECDSA |
| Key passphrase change | Medium | Update passphrase |
| Agent management | Low | Add/remove from agent |

### 📋 Enhanced Search & Filter (v0.4.0)

| Feature | Priority | Description |
|---------|----------|-------------|
| Host search | High | Filter host list |
| Tag filtering | High | Filter by tags |
| Fuzzy matching | Medium | Smart search |
| Quick connect | Medium | Type hostname directly |

### 📋 SSH Config Import (v0.4.0)

| Feature | Priority | Description |
|---------|----------|-------------|
| Parse ~/.ssh/config | High | Import existing hosts |
| Identity file mapping | High | IdentityFile → KeyFile auth |
| ProxyJump mapping | High | Map to JumpHost proxy |
| ProxyCommand mapping | Medium | Custom command support |
| Merge/update | Medium | Update existing hosts |

### 📋 Clipboard Support (v0.4.0)

| Feature | Priority | Description |
|---------|----------|-------------|
| Copy terminal selection | ✅ Done | Mouse selection → clipboard |
| Paste from clipboard | High | Clipboard → terminal |
| OSC 52 support | Medium | Remote copy to local clipboard |

### 📋 Session Tabs (v0.5.0)

| Feature | Priority | Description |
|---------|----------|-------------|
| Tab bar UI | High | Visual tab strip |
| Tab reordering | Medium | Drag and drop |
| Tab naming | Medium | Custom tab labels |
| Tab groups | Low | Group related sessions |

### 📋 Session Logging (v0.5.0)

| Feature | Priority | Description |
|---------|----------|-------------|
| Raw session logging | High | Save terminal output |
| Timestamped logging | High | Add timestamps |
| Log rotation | Medium | Size-based rotation |
| Log export | Medium | Export to file |

---

## Future Considerations

### 🔮 Advanced Features

| Feature | Complexity | Description |
|---------|------------|-------------|
| Split panes | High | Tmux-style splits |
| Session sharing | High | Collaborative sessions |
| X11 forwarding | Medium | Graphics forwarding |
| Agent forwarding | Medium | SSH agent forwarding |
| Custom themes | Low | User-defined themes |
| Plugin system | High | Extensibility |

### 🔮 Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| Linux | ✅ Primary | Full support |
| macOS | ✅ Primary | Full support |
| Windows | 📋 Future | Needs testing, libssh2 compat |
| WSL | 📋 Future | Should work, needs testing |

---

## Version Roadmap

### v0.1.0 (Current)
- Core TUI and connection management
- SSH sessions with VT100 emulation
- Multi-session support
- Basic SFTP browser
- Credential management
- Proxy support (Jump, SOCKS, HTTP, ProxyCommand)

### v0.2.0 (Next)
- Complete SFTP file operations
- Transfer queue with progress
- Improved error handling
- Performance optimizations

### v0.3.0
- Tunnel management UI
- Key management UI
- Connection history

### v0.4.0
- `~/.ssh/config` import
- Host search and filtering
- Enhanced clipboard support

### v0.5.0
- Session tabs
- Session logging
- Custom themes

---

## Technical Debt

| Item | Priority | Notes |
|------|----------|-------|
| Test coverage | High | Expand integration tests |
| Error messages | Medium | More user-friendly errors |
| Documentation | Medium | API docs, examples |
| Code refactoring | Low | `state.rs` is large (~2500 lines) |
| Accessibility | Low | Screen reader support |

---

## Contributing

Areas where contributions are welcome:

1. **Testing** - More integration tests, edge cases
2. **Documentation** - Usage examples, screenshots
3. **Features** - Any items from the planned list
4. **Bug fixes** - Issue reports and fixes
5. **Platform testing** - Especially Windows/WSL

See [README.md](../README.md) for contribution guidelines.
