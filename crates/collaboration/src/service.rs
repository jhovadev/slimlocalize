use std::sync::Arc;

use base64::Engine;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use slimlocalize_core::collab::{CollabDocumentRecord, CollabStore, PeerIdentity};
use slimlocalize_core::ports::Store;
use slimlocalize_core::{CheckReport, DomainError, LocalizeService, Result, TranslationEntry};
use uuid::Uuid;

use crate::document::TranslationDocument;

pub const PROTOCOL_VERSION: u16 = 1;
pub const SCHEMA_VERSION: i32 = 1;
pub const DOCUMENT_KEY_PREFIX: &str = "project";
pub const MAX_ENVELOPE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CollabStatus {
    pub project_id: Uuid,
    pub document_id: Uuid,
    pub peer_id: u64,
    pub document_key: String,
    pub version_vector: Vec<u8>,
    pub update_count: usize,
    pub has_pending: bool,
    pub schema_version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncEnvelope {
    pub format: String,
    pub protocol_version: u16,
    pub schema_version: i32,
    pub project_id: Uuid,
    pub document_key: String,
    pub peer_id: u64,
    pub version_vector: String,
    pub payload: String,
    pub sha256: String,
}

impl SyncEnvelope {
    pub fn from_update(
        project_id: Uuid,
        document_key: String,
        peer_id: u64,
        version_vector: Vec<u8>,
        payload: Vec<u8>,
    ) -> Self {
        let digest = Sha256::digest(&payload);
        Self {
            format: "slimlocalize-sync".to_owned(),
            protocol_version: PROTOCOL_VERSION,
            schema_version: SCHEMA_VERSION,
            project_id,
            document_key,
            peer_id,
            version_vector: base64::engine::general_purpose::STANDARD.encode(version_vector),
            payload: base64::engine::general_purpose::STANDARD.encode(payload),
            sha256: hex::encode(digest),
        }
    }

    pub fn validate(&self, expected_project: Uuid) -> Result<Vec<u8>> {
        if self.format != "slimlocalize-sync" {
            return Err(DomainError::validation("format", "unsupported sync format"));
        }
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(DomainError::validation(
                "protocolVersion",
                "unsupported protocol version",
            ));
        }
        if self.schema_version != SCHEMA_VERSION {
            return Err(DomainError::validation(
                "schemaVersion",
                "incompatible document schema",
            ));
        }
        if self.project_id != expected_project {
            return Err(DomainError::validation(
                "projectId",
                "sync document belongs to another project",
            ));
        }
        let payload = base64::engine::general_purpose::STANDARD
            .decode(&self.payload)
            .map_err(|_| DomainError::validation("payload", "invalid base64"))?;
        if payload.len() > MAX_ENVELOPE_BYTES {
            return Err(DomainError::LimitExceeded {
                context: "sync payload".to_owned(),
                limit: MAX_ENVELOPE_BYTES as u64,
            });
        }
        let digest = Sha256::digest(&payload);
        if hex::encode(digest) != self.sha256 {
            return Err(DomainError::validation("sha256", "checksum mismatch"));
        }
        Ok(payload)
    }
}

#[derive(Debug)]
pub struct CollaborationService<S>
where
    S: Store + CollabStore + Send + Sync,
{
    store: Arc<S>,
    local_service: Arc<LocalizeService<S>>,
    device_name: String,
}

impl<S> CollaborationService<S>
where
    S: Store + CollabStore + Send + Sync,
{
    pub fn new(store: Arc<S>, device_name: impl Into<String>) -> Self {
        Self {
            store: Arc::clone(&store),
            local_service: Arc::new(LocalizeService::with_shared_store(store)),
            device_name: device_name.into(),
        }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn local_service(&self) -> &LocalizeService<S> {
        &self.local_service
    }

    pub fn document_key(project_id: Uuid) -> String {
        format!("{DOCUMENT_KEY_PREFIX}:{project_id}")
    }

    fn peer(&self) -> Result<PeerIdentity> {
        self.store.get_or_create_peer(&self.device_name)
    }

    pub fn open_document(
        &self,
        project_id: Uuid,
    ) -> Result<(CollabDocumentRecord, TranslationDocument)> {
        self.local_service.get_project(project_id)?;
        let peer = self.peer()?;
        let key = Self::document_key(project_id);
        let record = match self.store.get_document(project_id, &key)? {
            Some(record) => record,
            None => {
                let now = Utc::now();
                let record = CollabDocumentRecord {
                    id: Uuid::new_v4(),
                    project_id,
                    document_key: key.clone(),
                    schema_version: SCHEMA_VERSION,
                    peer_id: peer.peer_id,
                    created_at: now,
                    updated_at: now,
                };
                self.store.create_document(&record)?;
                record
            }
        };

        let document = TranslationDocument::new(peer.peer_id)?;
        if let Some((snapshot, pending, _)) = self.store.load_snapshot(record.id)? {
            document.import_updates(&snapshot)?;
            if let Some(pending) = pending {
                let _ = document.import_updates(&pending);
            }
        }
        for update in self.store.list_updates(record.id)? {
            let _ = document.import_updates(&update.update);
        }
        self.persist_document(record.id, &document)?;
        Ok((record, document))
    }

    fn persist_document(&self, document_id: Uuid, document: &TranslationDocument) -> Result<()> {
        let snapshot = document.snapshot()?;
        let pending = document.export_pending_updates()?;
        let version_vector = document.state_vector();
        let now = Utc::now();
        self.store.save_snapshot(
            document_id,
            &snapshot,
            pending.as_deref(),
            &version_vector,
            now,
        )?;
        self.store.save_pending(document_id, pending.as_deref())?;
        self.store
            .save_checkpoint(document_id, &version_vector, now)?;
        self.store.touch_document(document_id, now)?;
        Ok(())
    }

    pub fn sync_from_sql(&self, project_id: Uuid) -> Result<CollabStatus> {
        let (_record, document) = self.open_document(project_id)?;
        for entry in self.local_service.list_project_translations(project_id)? {
            if let Some(value) = entry.value {
                document.set_translation(
                    &entry.namespace,
                    &entry.key,
                    &entry.locale_code,
                    &value,
                )?;
            }
        }
        let update = document.export_updates()?;
        let record = self
            .store
            .get_document(project_id, &Self::document_key(project_id))?
            .unwrap();
        if !update.is_empty() {
            self.store.append_update(
                record.id,
                &update,
                Some(&record.peer_id.to_string()),
                Utc::now(),
            )?;
        }
        self.persist_document(record.id, &document)?;
        self.status(project_id)
    }

    pub fn bootstrap(&self, project_id: Uuid) -> Result<CollabStatus> {
        let (record, document) = self.open_document(project_id)?;
        if document.translations().is_empty() {
            for entry in self.local_service.list_project_translations(project_id)? {
                if let Some(value) = entry.value {
                    document.set_translation(
                        &entry.namespace,
                        &entry.key,
                        &entry.locale_code,
                        &value,
                    )?;
                }
            }
            let update = document.export_updates()?;
            if !update.is_empty() {
                self.store.append_update(
                    record.id,
                    &update,
                    Some(&record.peer_id.to_string()),
                    Utc::now(),
                )?;
            }
            self.persist_document(record.id, &document)?;
        }
        self.status(project_id)
    }

    pub fn set_translation(
        &self,
        project_id: Uuid,
        namespace: &str,
        key: &str,
        locale: &str,
        value: &str,
        status: slimlocalize_core::TranslationStatus,
    ) -> Result<TranslationEntry> {
        let (record, document) = self.open_document(project_id)?;
        let before = document.state_vector();
        document.set_translation(namespace, key, locale, value)?;
        let update = document.export_updates_since(&before)?;
        if !update.is_empty() {
            self.store.append_update(
                record.id,
                &update,
                Some(&record.peer_id.to_string()),
                Utc::now(),
            )?;
        }
        let entry =
            self.local_service
                .save_translation(slimlocalize_core::SaveTranslationInput {
                    project_id,
                    namespace: namespace.to_owned(),
                    key: key.to_owned(),
                    locale_code: locale.to_owned(),
                    value: value.to_owned(),
                    status,
                    description: None,
                })?;
        self.persist_document(record.id, &document)?;
        Ok(entry)
    }

    pub fn export_update(&self, project_id: Uuid) -> Result<SyncEnvelope> {
        let (record, document) = self.open_document(project_id)?;
        let payload = document.export_updates()?;
        if payload.len() > MAX_ENVELOPE_BYTES {
            return Err(DomainError::LimitExceeded {
                context: "sync payload".to_owned(),
                limit: MAX_ENVELOPE_BYTES as u64,
            });
        }
        Ok(SyncEnvelope::from_update(
            project_id,
            record.document_key,
            record.peer_id,
            document.state_vector(),
            payload,
        ))
    }

    pub fn import_update(&self, project_id: Uuid, envelope: &SyncEnvelope) -> Result<()> {
        let payload = envelope.validate(project_id)?;
        let (record, document) = self.open_document(project_id)?;
        let status = document.import_updates(&payload)?;
        let origin = envelope.peer_id.to_string();
        self.store
            .append_update(record.id, &payload, Some(&origin), Utc::now())?;
        if status.pending.is_some() {
            if let Some(pending) = document.export_pending_updates()? {
                self.store.save_pending(record.id, Some(&pending))?;
            }
        } else {
            self.store.save_pending(record.id, None)?;
        }
        self.persist_document(record.id, &document)?;
        self.rebuild_projection(project_id)?;
        Ok(())
    }

    pub fn status(&self, project_id: Uuid) -> Result<CollabStatus> {
        let (record, document) = self.open_document(project_id)?;
        let updates = self.store.list_updates(record.id)?;
        let pending = self
            .store
            .load_snapshot(record.id)?
            .and_then(|(_, pending, _)| pending)
            .is_some();
        Ok(CollabStatus {
            project_id,
            document_id: record.id,
            peer_id: record.peer_id,
            document_key: record.document_key,
            version_vector: document.state_vector(),
            update_count: updates.len(),
            has_pending: pending,
            schema_version: record.schema_version,
        })
    }

    pub fn rebuild_projection(&self, project_id: Uuid) -> Result<()> {
        let (_record, document) = self.open_document(project_id)?;
        for entry in document.translations() {
            let _ = self
                .local_service
                .save_translation(slimlocalize_core::SaveTranslationInput {
                    project_id,
                    namespace: entry.namespace,
                    key: entry.key,
                    locale_code: entry.locale,
                    value: entry.value,
                    status: slimlocalize_core::TranslationStatus::Translated,
                    description: None,
                });
        }
        Ok(())
    }

    pub fn validate(&self, project_id: Uuid) -> Result<CheckReport> {
        self.local_service.check_project(project_id)
    }

    pub fn list_project_translations(&self, project_id: Uuid) -> Result<Vec<TranslationEntry>> {
        self.local_service.list_project_translations(project_id)
    }
}
