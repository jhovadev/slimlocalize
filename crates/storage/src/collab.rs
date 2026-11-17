use chrono::{DateTime, Utc};
use slimlocalize_core::collab::{CollabDocumentRecord, CollabStore, PeerIdentity, StoredUpdate};
use slimlocalize_core::{DomainError, Result};
use uuid::Uuid;

use crate::migrations::map_db;
use crate::store::SqliteStore;
use rusqlite::OptionalExtension;

impl CollabStore for SqliteStore {
    fn get_or_create_peer(&self, device_name: &str) -> Result<PeerIdentity> {
        let connection = self.lock()?;
        let existing = connection
            .query_row(
                "SELECT peer_id, device_name, created_at FROM collab_peers ORDER BY created_at LIMIT 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(map_db("read collab peer"))?;
        if let Some((peer_id, device_name, created_at)) = existing {
            return Ok(PeerIdentity {
                peer_id: peer_id
                    .parse()
                    .map_err(|_| DomainError::storage("invalid persisted peer id"))?,
                device_name,
                created_at: parse_dt(&created_at)?,
            });
        }
        drop(connection);
        let peer_id: u64 = (Uuid::new_v4().as_u128() as u64) | 1;
        let created_at = Utc::now();
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO collab_peers (peer_id, device_name, created_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![peer_id.to_string(), device_name, created_at.to_rfc3339()],
            )
            .map_err(map_db("insert collab peer"))?;
        Ok(PeerIdentity {
            peer_id,
            device_name: device_name.to_owned(),
            created_at,
        })
    }

    fn get_document(
        &self,
        project_id: Uuid,
        document_key: &str,
    ) -> Result<Option<CollabDocumentRecord>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT id, project_id, document_key, schema_version, peer_id, created_at, updated_at
                 FROM collab_documents WHERE project_id = ?1 AND document_key = ?2",
                rusqlite::params![project_id.to_string(), document_key],
                |row| {
                    Ok(CollabDocumentRecord {
                        id: parse_uuid(row, 0)?,
                        project_id: parse_uuid(row, 1)?,
                        document_key: row.get(2)?,
                        schema_version: row.get(3)?,
                        peer_id: row
                            .get::<_, String>(4)?
                            .parse()
                            .map_err(|_| invalid_col(4, "peer id"))?,
                        created_at: parse_dt_sqlite(5, row.get::<_, String>(5)?)?,
                        updated_at: parse_dt_sqlite(6, row.get::<_, String>(6)?)?,
                    })
                },
            )
            .optional()
            .map_err(map_db("read collab document"))
    }

    fn create_document(&self, record: &CollabDocumentRecord) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO collab_documents
                 (id, project_id, document_key, schema_version, peer_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    record.id.to_string(),
                    record.project_id.to_string(),
                    record.document_key,
                    record.schema_version,
                    record.peer_id.to_string(),
                    record.created_at.to_rfc3339(),
                    record.updated_at.to_rfc3339(),
                ],
            )
            .map_err(map_db("insert collab document"))?;
        Ok(())
    }

    fn touch_document(&self, document_id: Uuid, at: DateTime<Utc>) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "UPDATE collab_documents SET updated_at = ?1 WHERE id = ?2",
                rusqlite::params![at.to_rfc3339(), document_id.to_string()],
            )
            .map_err(map_db("touch collab document"))?;
        Ok(())
    }

    fn save_snapshot(
        &self,
        document_id: Uuid,
        snapshot: &[u8],
        pending: Option<&[u8]>,
        version_vector: &[u8],
        created_at: DateTime<Utc>,
    ) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO collab_snapshots
                 (document_id, snapshot, pending, version_vector, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(document_id) DO UPDATE SET
                    snapshot = excluded.snapshot,
                    pending = excluded.pending,
                    version_vector = excluded.version_vector,
                    created_at = excluded.created_at",
                rusqlite::params![
                    document_id.to_string(),
                    snapshot,
                    pending,
                    version_vector,
                    created_at.to_rfc3339(),
                ],
            )
            .map_err(map_db("save collab snapshot"))?;
        Ok(())
    }

    fn load_snapshot(
        &self,
        document_id: Uuid,
    ) -> Result<Option<(Vec<u8>, Option<Vec<u8>>, Vec<u8>)>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT snapshot, pending, version_vector FROM collab_snapshots WHERE document_id = ?1",
                rusqlite::params![document_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, Option<Vec<u8>>>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(map_db("load collab snapshot"))
    }

    fn append_update(
        &self,
        document_id: Uuid,
        update: &[u8],
        origin_peer_id: Option<&str>,
        created_at: DateTime<Utc>,
    ) -> Result<i64> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO collab_updates (document_id, update_blob, origin_peer_id, created_at, applied)
                 VALUES (?1, ?2, ?3, ?4, 1)",
                rusqlite::params![
                    document_id.to_string(),
                    update,
                    origin_peer_id,
                    created_at.to_rfc3339(),
                ],
            )
            .map_err(map_db("append collab update"))?;
        Ok(connection.last_insert_rowid())
    }

    fn list_updates(&self, document_id: Uuid) -> Result<Vec<StoredUpdate>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, update_blob, origin_peer_id, created_at
                 FROM collab_updates WHERE document_id = ?1 ORDER BY id",
            )
            .map_err(map_db("prepare collab updates"))?;
        let rows = statement
            .query_map(rusqlite::params![document_id.to_string()], |row| {
                Ok(StoredUpdate {
                    id: row.get(0)?,
                    update: row.get(1)?,
                    origin_peer_id: row.get(2)?,
                    created_at: parse_dt_sqlite(3, row.get::<_, String>(3)?)?,
                })
            })
            .map_err(map_db("list collab updates"))?;
        let mut values = Vec::new();
        for row in rows {
            values.push(row.map_err(map_db("read collab update"))?);
        }
        Ok(values)
    }

    fn save_pending(&self, document_id: Uuid, pending: Option<&[u8]>) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "UPDATE collab_snapshots SET pending = ?1 WHERE document_id = ?2",
                rusqlite::params![pending, document_id.to_string()],
            )
            .map_err(map_db("save collab pending"))?;
        Ok(())
    }

    fn save_checkpoint(
        &self,
        document_id: Uuid,
        version_vector: &[u8],
        updated_at: DateTime<Utc>,
    ) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO collab_checkpoints (document_id, version_vector, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(document_id) DO UPDATE SET
                    version_vector = excluded.version_vector,
                    updated_at = excluded.updated_at",
                rusqlite::params![
                    document_id.to_string(),
                    version_vector,
                    updated_at.to_rfc3339()
                ],
            )
            .map_err(map_db("save collab checkpoint"))?;
        Ok(())
    }

    fn load_checkpoint(&self, document_id: Uuid) -> Result<Option<Vec<u8>>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT version_vector FROM collab_checkpoints WHERE document_id = ?1",
                rusqlite::params![document_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_db("load collab checkpoint"))
    }
}

fn parse_uuid(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Uuid> {
    let value: String = row.get(index)?;
    Uuid::parse_str(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn parse_dt(value: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| DomainError::storage(format!("invalid timestamp {value}: {error}")))
}

fn invalid_col(index: usize, message: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message,
        )),
    )
}

fn parse_dt_sqlite(index: usize, value: String) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                index,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
}
