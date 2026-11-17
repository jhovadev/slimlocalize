//! Security primitives and explicit contracts for capabilities that are not yet
//! implemented.
//!
//! Implemented in F1:
//! - path traversal protection for imports/exports (via the core path helpers),
//! - secret redaction types,
//! - an authorization *model* (not enforced by the local app yet),
//! - the shape of an encrypted vault envelope.
//!
//! **Not implemented:** encryption, key derivation, key storage or remote
//! authorization. No hardcoded key is provided because that would be insecure.

use std::fmt;

use serde::{Deserialize, Serialize};
use slimlocalize_core::{DomainError, MemberRole, Result};
use uuid::Uuid;

pub use slimlocalize_core::paths::{ensure_within, safe_join};

#[derive(Clone, PartialEq, Eq)]
pub struct SecretString(String);

impl SecretString {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Explicit, auditable access. Callers should keep the value in memory only
    /// for the shortest possible time and never log it.
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretString([REDACTED])")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        // Best-effort zeroization of the allocation. Rust does not guarantee
        // that no copies exist, so this is defence in depth, not a guarantee.
        use zeroize::Zeroize;
        self.0.zeroize();
    }
}

/// Sanitize a map before logging. Keys that look sensitive are replaced.
#[must_use]
pub fn redact_sensitive_pairs(
    pairs: impl IntoIterator<Item = (String, String)>,
) -> Vec<(String, String)> {
    pairs
        .into_iter()
        .map(|(key, value)| {
            let sensitive = ["SECRET", "TOKEN", "PASSWORD", "PRIVATE_KEY", "API_KEY"]
                .iter()
                .any(|needle| key.to_ascii_uppercase().contains(needle));
            let value = if sensitive {
                "[REDACTED]".to_owned()
            } else {
                value
            };
            (key, value)
        })
        .collect()
}

/// Marker for a secret stored in a future encrypted vault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretRef {
    pub id: Uuid,
    pub label: String,
    pub environment: String,
}

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("vault is not implemented in this release")]
    NotImplemented,
    #[error("secret not found")]
    NotFound,
    #[error("key provider unavailable: {0}")]
    KeyProvider(String),
    #[error("cryptographic failure: {0}")]
    Crypto(String),
}

/// Contract for a future encrypted secret store.
///
/// Implementations must use authenticated encryption with a random nonce per
/// write, a user-controlled key (OS keyring or passphrase-derived key), and
/// must never persist plaintext or put secrets in Loro documents.
pub trait SecretVault: Send + Sync {
    fn put(&self, label: &str, value: &SecretString) -> std::result::Result<SecretRef, VaultError>;
    fn get(&self, reference: &SecretRef) -> std::result::Result<SecretString, VaultError>;
    fn delete(&self, reference: &SecretRef) -> std::result::Result<(), VaultError>;
}

/// Versioned envelope shape reserved for the future vault migration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultEnvelope {
    pub version: u8,
    pub algorithm: String,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub key_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    ReadWorkspace,
    WriteTranslations,
    ManageLocales,
    ManageConfiguration,
    ManageMembers,
    ManageSecrets,
}

impl Permission {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadWorkspace => "read_workspace",
            Self::WriteTranslations => "write_translations",
            Self::ManageLocales => "manage_locales",
            Self::ManageConfiguration => "manage_configuration",
            Self::ManageMembers => "manage_members",
            Self::ManageSecrets => "manage_secrets",
        }
    }
}

/// Pure authorization policy for the future collaboration layer.
///
/// This is a design contract and is **not enforced** by F1 adapters. The local
/// app has a single implicit owner. Remote MCP and sharing must remain disabled
/// until this policy is backed by authenticated identities.
#[derive(Debug, Clone, Copy, Default)]
pub struct AuthorizationPolicy;

impl AuthorizationPolicy {
    #[must_use]
    pub const fn allows(role: MemberRole, permission: Permission) -> bool {
        match role {
            MemberRole::Owner => true,
            MemberRole::Maintainer => matches!(
                permission,
                Permission::ReadWorkspace
                    | Permission::WriteTranslations
                    | Permission::ManageLocales
                    | Permission::ManageConfiguration
            ),
            MemberRole::Translator => matches!(
                permission,
                Permission::ReadWorkspace | Permission::WriteTranslations
            ),
            MemberRole::Viewer => matches!(permission, Permission::ReadWorkspace),
        }
    }

    pub fn require(role: MemberRole, permission: Permission) -> Result<()> {
        if Self::allows(role, permission) {
            Ok(())
        } else {
            Err(DomainError::validation(
                "authorization",
                format!(
                    "role {} cannot perform {}",
                    role.as_str(),
                    permission.as_str()
                ),
            ))
        }
    }
}

/// Validate an export target inside an explicit root. Thin wrapper used by
/// adapters so the security boundary has a named home.
pub fn safe_export_path(root: &std::path::Path, file_name: &str) -> Result<std::path::PathBuf> {
    safe_join(root, file_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use slimlocalize_core::MemberRole;

    #[test]
    fn secrets_are_redacted_in_debug_and_display() {
        let secret = SecretString::new("super-secret");
        assert_eq!(format!("{secret:?}"), "SecretString([REDACTED])");
        assert_eq!(format!("{secret}"), "[REDACTED]");
        assert_eq!(secret.expose_secret(), "super-secret");
    }

    #[test]
    fn authorization_policy_matches_roles() {
        assert!(AuthorizationPolicy::allows(
            MemberRole::Owner,
            Permission::ManageSecrets
        ));
        assert!(AuthorizationPolicy::allows(
            MemberRole::Translator,
            Permission::WriteTranslations
        ));
        assert!(!AuthorizationPolicy::allows(
            MemberRole::Translator,
            Permission::ManageMembers
        ));
        assert!(!AuthorizationPolicy::allows(
            MemberRole::Viewer,
            Permission::WriteTranslations
        ));
    }

    #[test]
    fn redacts_sensitive_log_pairs() {
        let pairs = redact_sensitive_pairs([
            ("PUBLIC_URL".to_owned(), "http://localhost".to_owned()),
            ("API_TOKEN".to_owned(), "abc".to_owned()),
        ]);
        assert_eq!(pairs[0].1, "http://localhost");
        assert_eq!(pairs[1].1, "[REDACTED]");
    }

    #[test]
    fn export_paths_cannot_escape() {
        let dir = tempfile::tempdir().unwrap();
        assert!(safe_export_path(dir.path(), "en-US.json").is_ok());
        assert!(safe_export_path(dir.path(), "../outside.json").is_err());
    }
}
