//! Durable local-first collaboration for SlimLocalize.

pub mod document;
pub mod service;

pub use document::TranslationDocument;
pub use service::{
    CollabStatus, CollaborationService, PROTOCOL_VERSION, SCHEMA_VERSION, SyncEnvelope,
};
