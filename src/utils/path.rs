//! Path resolution utilities

use std::path::{Path, PathBuf};

/// Resolve SSH key path supporting:
/// - Full paths: "/home/user/.ssh/id_rsa" (unchanged)
/// - Tilde paths: "~/.ssh/id_rsa" (expanded)
/// - Key names: "id_rsa" (resolved to ~/.ssh/id_rsa)
pub fn resolve_ssh_key_path(path: &Path) -> PathBuf {
    let path_str = path.to_string_lossy();

    if path.is_absolute() {
        return path.to_path_buf();
    }

    if let Some(stripped) = path_str.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }

    // Key name only - resolve to ~/.ssh/{name}
    if !path_str.contains('/') && !path_str.contains('\\') {
        if let Some(home) = dirs::home_dir() {
            return home.join(".ssh").join(path);
        }
    }

    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_absolute_path_unchanged() {
        let path = Path::new("/home/user/.ssh/id_rsa");
        assert_eq!(resolve_ssh_key_path(path), path.to_path_buf());
    }

    #[test]
    fn test_tilde_path_expanded() {
        let path = Path::new("~/.ssh/id_rsa");
        let resolved = resolve_ssh_key_path(path);
        assert!(!resolved.to_string_lossy().starts_with("~"));
        assert!(resolved.to_string_lossy().ends_with(".ssh/id_rsa"));
    }

    #[test]
    fn test_key_name_resolved() {
        let path = Path::new("id_ed25519");
        let resolved = resolve_ssh_key_path(path);
        assert!(resolved.to_string_lossy().contains(".ssh"));
        assert!(resolved.to_string_lossy().ends_with("id_ed25519"));
    }
}
