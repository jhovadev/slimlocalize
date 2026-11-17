use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Persistent peer identity used by collaborative documents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerIdentity {
    pub peer_id: u64,
    pub device_name: String,
    pub created_at: DateTime<Utc>,
}

/// Metadata for one collaborative document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollabDocumentRecord {
    pub id: Uuid,
    pub project_id: Uuid,
    pub document_key: String,
    pub schema_version: i32,
    pub peer_id: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// One persisted Loro update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredUpdate {
    pub id: i64,
    pub update: Vec<u8>,
    pub origin_peer_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Persistence contract for collaborative documents.
///
/// Implemented by the SQLite adapter. The collaboration crate depends on this
/// contract instead of SQL.
pub type SnapshotData = (Vec<u8>, Option<Vec<u8>>, Vec<u8>);

pub trait CollabStore: Send + Sync {
    fn get_or_create_peer(&self, device_name: &str) -> crate::Result<PeerIdentity>;
    fn get_document(
        &self,
        project_id: Uuid,
        document_key: &str,
    ) -> crate::Result<Option<CollabDocumentRecord>>;
    fn create_document(&self, record: &CollabDocumentRecord) -> crate::Result<()>;
    fn touch_document(&self, document_id: Uuid, at: DateTime<Utc>) -> crate::Result<()>;
    fn save_snapshot(
        &self,
        document_id: Uuid,
        snapshot: &[u8],
        pending: Option<&[u8]>,
        version_vector: &[u8],
        created_at: DateTime<Utc>,
    ) -> crate::Result<()>;
    fn load_snapshot(&self, document_id: Uuid) -> crate::Result<Option<SnapshotData>>;
    fn append_update(
        &self,
        document_id: Uuid,
        update: &[u8],
        origin_peer_id: Option<&str>,
        created_at: DateTime<Utc>,
    ) -> crate::Result<i64>;
    fn list_updates(&self, document_id: Uuid) -> crate::Result<Vec<StoredUpdate>>;
    fn save_pending(&self, document_id: Uuid, pending: Option<&[u8]>) -> crate::Result<()>;
    fn save_checkpoint(
        &self,
        document_id: Uuid,
        version_vector: &[u8],
        updated_at: DateTime<Utc>,
    ) -> crate::Result<()>;
    fn load_checkpoint(&self, document_id: Uuid) -> crate::Result<Option<Vec<u8>>>;
}
