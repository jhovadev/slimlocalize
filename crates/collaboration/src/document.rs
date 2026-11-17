use loro::{ExportMode, ImportStatus, LoroDoc, LoroValue, ToJson, ValueOrContainer, VersionVector};
use serde::{Deserialize, Serialize};

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
        let composite = format!("{namespace}\u{1f}{key}\u{1f}{locale}");
        map.insert(&composite, value)?;
        self.doc.commit();
        Ok(())
    }

    pub fn get_translation(&self, namespace: &str, key: &str, locale: &str) -> Option<String> {
        let map = self.doc.get_map(TRANSLATION_CONTAINER);
        let composite = format!("{namespace}\u{1f}{key}\u{1f}{locale}");
        let value = map.get(&composite)?;
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

    pub fn export_pending_updates(&self) -> Result<Option<Vec<u8>>> {
        Ok(self.doc.export_pending_updates()?)
    }

    pub fn state_vector(&self) -> Vec<u8> {
        self.doc.oplog_vv().encode()
    }

    pub fn import_updates(&self, updates: &[u8]) -> Result<ImportStatus> {
        let status = self.doc.import(updates)?;
        self.doc.commit();
        Ok(status)
    }

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

impl From<CollaborationError> for slimlocalize_core::DomainError {
    fn from(error: CollaborationError) -> Self {
        match error {
            CollaborationError::Loro(error) => Self::storage(format!("Loro error: {error}")),
            CollaborationError::Encode(error) => {
                Self::storage(format!("Loro encode error: {error}"))
            }
            CollaborationError::InvalidInput(message) => Self::validation("collaboration", message),
        }
    }
}
