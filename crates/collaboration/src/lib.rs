//! Loro CRDT prototype for future collaboration.
//!
//! This crate is **F2 groundwork**, not a complete collaboration system. It
//! separates the CRDT document from persistence, transport and authorization.
//! The SQLite tables `sync_documents` / `sync_checkpoints` reserve opaque
//! payloads; no replication is enabled by the app or server yet.
//!
//! Authority rule: when collaboration is enabled, the CRDT document is the
//! authority for shared translation text and SQLite projections are derived.
//! Do not write the same fields through both paths.

use loro::{ExportMode, ImportStatus, LoroDoc, LoroValue, ToJson, ValueOrContainer, VersionVector};
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;
pub const TRANSLATION_CONTAINER: &str = "translations";

#[derive(Debug, thiserror::Error)]
pub enum CollaborationError {
    #[error("Loro error: {0}")]
    Loro(#[from] loro::LoroError),
    #[error("Loro encode error: {0}")]
    Encode(#[from] loro::LoroEncodeError),
    #[error("{0}")]
    InvalidInput(String),
}

pub type Result<T> = std::result::Result<T, CollaborationError>;

/// One replica of a translation document.
///
/// Keys use the unit separator (`\u{1f}`) so validated namespaces, keys and
/// locale codes cannot collide. That separator is rejected by domain
/// validation (control characters are not allowed in keys).
#[derive(Debug)]
pub struct TranslationDocument {
    doc: LoroDoc,
    peer_id: u64,
}

impl TranslationDocument {
    pub fn new(peer_id: u64) -> Result<Self> {
        let doc = LoroDoc::new();
        doc.set_peer_id(peer_id)?;
        Ok(Self { doc, peer_id })
    }

    pub fn restore(snapshot: &[u8], peer_id: u64) -> Result<Self> {
        let doc = LoroDoc::new();
        doc.set_peer_id(peer_id)?;
        let status = doc.import(snapshot)?;
        if status.pending.is_some() {
            return Err(CollaborationError::InvalidInput(
                "snapshot has unresolved pending updates".to_owned(),
            ));
        }
        Ok(Self { doc, peer_id })
    }

    #[must_use]
    pub fn peer_id(&self) -> u64 {
        self.peer_id
    }

    pub fn set_translation(
        &self,
        namespace: &str,
        key: &str,
        locale: &str,
        value: &str,
    ) -> Result<()> {
        validate_part(namespace, "namespace")?;
        validate_part(key, "key")?;
        validate_part(locale, "locale")?;
        let map = self.doc.get_map(TRANSLATION_CONTAINER);
        map.insert(&composite(namespace, key, locale), value)?;
        self.doc.commit();
        Ok(())
    }

    pub fn get_translation(&self, namespace: &str, key: &str, locale: &str) -> Option<String> {
        let map = self.doc.get_map(TRANSLATION_CONTAINER);
        let value = map.get(&composite(namespace, key, locale))?;
        match value.get_deep_value() {
            LoroValue::String(value) => Some(value.as_ref().to_owned()),
            _ => None,
        }
    }

    pub fn snapshot(&self) -> Result<Vec<u8>> {
        Ok(self.doc.export(ExportMode::Snapshot)?)
    }

    pub fn export_updates(&self) -> Result<Vec<u8>> {
        Ok(self.doc.export(ExportMode::all_updates())?)
    }

    pub fn export_updates_since(&self, state_vector: &[u8]) -> Result<Vec<u8>> {
        let version = VersionVector::decode(state_vector)?;
        Ok(self.doc.export(ExportMode::updates(&version))?)
    }

    /// Opaque, serializable version vector suitable for `SyncCheckpoint`.
    pub fn state_vector(&self) -> Vec<u8> {
        self.doc.oplog_vv().encode()
    }

    pub fn import_updates(&self, updates: &[u8]) -> Result<ImportStatus> {
        let status = self.doc.import(updates)?;
        self.doc.commit();
        Ok(status)
    }

    /// JSON projection used by convergence tests and debugging. This is not the
    /// persistence authority.
    pub fn deep_json(&self) -> serde_json::Value {
        self.doc.get_deep_value().to_json_value()
    }

    pub fn translations(&self) -> Vec<DocumentEntry> {
        let map = self.doc.get_map(TRANSLATION_CONTAINER);
        let mut entries = Vec::new();
        map.for_each(|key, value| {
            if let ValueOrContainer::Value(LoroValue::String(text)) = value
                && let Some((namespace, translation_key, locale)) = split_composite(key)
            {
                entries.push(DocumentEntry {
                    namespace: namespace.to_owned(),
                    key: translation_key.to_owned(),
                    locale: locale.to_owned(),
                    value: text.as_ref().to_owned(),
                });
            }
        });
        entries.sort_by(|left, right| {
            (&left.namespace, &left.key, &left.locale).cmp(&(
                &right.namespace,
                &right.key,
                &right.locale,
            ))
        });
        entries
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentEntry {
    pub namespace: String,
    pub key: String,
    pub locale: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncMessageKind {
    Snapshot,
    Updates,
    StateVector,
}

/// Transport-neutral envelope. Authentication and authorization are explicitly
/// out of scope: remote sync must not be exposed until both exist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncEnvelope {
    pub protocol_version: u16,
    pub document: String,
    pub peer_id: String,
    pub kind: SyncMessageKind,
    pub payload: Vec<u8>,
}

impl SyncEnvelope {
    #[must_use]
    pub fn new(peer_id: impl Into<String>, kind: SyncMessageKind, payload: Vec<u8>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            document: "translations".to_owned(),
            peer_id: peer_id.into(),
            kind,
            payload,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(CollaborationError::InvalidInput(format!(
                "unsupported protocol version {}",
                self.protocol_version
            )));
        }
        if self.document != "translations" {
            return Err(CollaborationError::InvalidInput(
                "unsupported document".to_owned(),
            ));
        }
        if self.payload.is_empty() {
            return Err(CollaborationError::InvalidInput(
                "empty sync payload".to_owned(),
            ));
        }
        Ok(())
    }
}

fn validate_part(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(CollaborationError::InvalidInput(format!(
            "{field} must not be empty"
        )));
    }
    if value.contains('\u{1f}') {
        return Err(CollaborationError::InvalidInput(format!(
            "{field} contains a reserved separator"
        )));
    }
    Ok(())
}

fn composite(namespace: &str, key: &str, locale: &str) -> String {
    format!("{namespace}\u{1f}{key}\u{1f}{locale}")
}

fn split_composite(value: &str) -> Option<(&str, &str, &str)> {
    let mut parts = value.split('\u{1f}');
    let namespace = parts.next()?;
    let key = parts.next()?;
    let locale = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some((namespace, key, locale))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_roundtrips_through_snapshot() {
        let document = TranslationDocument::new(1).unwrap();
        document
            .set_translation("common", "welcome", "en-US", "Welcome, {name}!")
            .unwrap();
        let snapshot = document.snapshot().unwrap();
        let restored = TranslationDocument::restore(&snapshot, 2).unwrap();
        assert_eq!(
            restored.get_translation("common", "welcome", "en-US"),
            Some("Welcome, {name}!".to_owned())
        );
        assert_eq!(restored.translations().len(), 1);
    }

    #[test]
    fn two_replicas_converge_after_concurrent_edits() {
        let base = TranslationDocument::new(1).unwrap();
        base.set_translation("common", "welcome", "en-US", "Hello")
            .unwrap();
        let snapshot = base.snapshot().unwrap();
        let base_state = base.state_vector();

        let alice = TranslationDocument::restore(&snapshot, 10).unwrap();
        let bob = TranslationDocument::restore(&snapshot, 20).unwrap();

        alice
            .set_translation("common", "welcome", "es-PE", "Hola")
            .unwrap();
        alice
            .set_translation("common", "save", "es-PE", "Guardar")
            .unwrap();
        bob.set_translation("common", "welcome", "es-PE", "Buenas")
            .unwrap();
        bob.set_translation("common", "cancel", "es-PE", "Cancelar")
            .unwrap();

        let alice_updates = alice.export_updates_since(&base_state).unwrap();
        let bob_updates = bob.export_updates_since(&base_state).unwrap();
        assert!(!alice_updates.is_empty());
        assert!(!bob_updates.is_empty());

        let alice_status = alice.import_updates(&bob_updates).unwrap();
        let bob_status = bob.import_updates(&alice_updates).unwrap();
        assert!(alice_status.pending.is_none());
        assert!(bob_status.pending.is_none());
        assert_eq!(alice.deep_json(), bob.deep_json());
        assert_eq!(
            alice.get_translation("common", "save", "es-PE"),
            Some("Guardar".to_owned())
        );
        assert_eq!(
            bob.get_translation("common", "cancel", "es-PE"),
            Some("Cancelar".to_owned())
        );
        // Concurrent writes to the same map key resolve deterministically.
        assert_eq!(
            alice.get_translation("common", "welcome", "es-PE"),
            bob.get_translation("common", "welcome", "es-PE")
        );
    }

    #[test]
    fn imports_are_idempotent() {
        let alice = TranslationDocument::new(1).unwrap();
        alice
            .set_translation("common", "welcome", "en-US", "Hello")
            .unwrap();
        let updates = alice.export_updates().unwrap();

        let bob = TranslationDocument::new(2).unwrap();
        bob.import_updates(&updates).unwrap();
        let first = bob.deep_json();
        let status = bob.import_updates(&updates).unwrap();
        assert!(status.pending.is_none());
        assert_eq!(bob.deep_json(), first);
        assert_eq!(
            bob.get_translation("common", "welcome", "en-US"),
            Some("Hello".to_owned())
        );
    }

    #[test]
    fn sync_envelope_validates_protocol() {
        let envelope = SyncEnvelope::new("peer-1", SyncMessageKind::Updates, vec![1, 2, 3]);
        envelope.validate().unwrap();
        let mut invalid = envelope.clone();
        invalid.protocol_version = 99;
        assert!(invalid.validate().is_err());
        let empty = SyncEnvelope::new("peer-1", SyncMessageKind::StateVector, Vec::new());
        assert!(empty.validate().is_err());
    }

    #[test]
    fn rejects_reserved_separator() {
        let document = TranslationDocument::new(1).unwrap();
        assert!(
            document
                .set_translation("common", "bad\u{1f}key", "en-US", "value")
                .is_err()
        );
    }
}
