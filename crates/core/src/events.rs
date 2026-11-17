use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Domain events describe facts that already happened. F1 persists them as
/// audit rows; transports can consume the same shape later without coupling the
/// domain to a message broker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DomainEvent {
    WorkspaceCreated {
        workspace_id: Uuid,
        slug: String,
    },
    ProjectCreated {
        project_id: Uuid,
        workspace_id: Uuid,
        slug: String,
    },
    LocaleAdded {
        project_id: Uuid,
        code: String,
    },
    NamespaceCreated {
        project_id: Uuid,
        name: String,
    },
    TranslationSaved {
        project_id: Uuid,
        key: String,
        locale: String,
    },
    EnvironmentCreated {
        project_id: Uuid,
        name: String,
    },
    ConfigVariableSet {
        environment_id: Uuid,
        key: String,
    },
    TranslationsImported {
        project_id: Uuid,
        files: usize,
    },
    TranslationsExported {
        project_id: Uuid,
        files: usize,
    },
}

pub trait EventPublisher {
    fn publish(&self, event: &DomainEvent) -> crate::Result<()>;
}

/// Default publisher used by local adapters that only need audit persistence.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopPublisher;

impl EventPublisher for NoopPublisher {
    fn publish(&self, _event: &DomainEvent) -> crate::Result<()> {
        Ok(())
    }
}
