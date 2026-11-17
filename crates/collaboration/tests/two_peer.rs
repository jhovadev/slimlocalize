use std::path::Path;

use slimlocalize_collaboration::CollaborationService;
use slimlocalize_core::{
    AddLocaleInput, CreateProjectInput, CreateWorkspaceInput, LocalizeService,
    SaveTranslationInput, TranslationStatus,
};
use slimlocalize_storage::SqliteStore;
use tempfile::TempDir;

fn service(db: &Path) -> LocalizeService<SqliteStore> {
    LocalizeService::new(SqliteStore::open(db).unwrap())
}

#[test]
fn two_peers_converge_and_persist() {
    let dir = TempDir::new().unwrap();
    let a_db = dir.path().join("peer-a.sqlite3");
    let b_db = dir.path().join("peer-b.sqlite3");

    let service_a = service(&a_db);
    let workspace = service_a
        .create_workspace(CreateWorkspaceInput {
            name: "Sync".to_owned(),
            slug: None,
            description: None,
        })
        .unwrap();
    let project = service_a
        .create_project(
            workspace.id,
            CreateProjectInput {
                name: "App".to_owned(),
                slug: None,
                description: None,
                default_locale: Some("en-US".to_owned()),
            },
        )
        .unwrap();
    service_a
        .add_locale(
            project.id,
            AddLocaleInput {
                code: "es-PE".to_owned(),
                name: None,
                is_default: false,
            },
        )
        .unwrap();
    service_a
        .save_translation(SaveTranslationInput {
            project_id: project.id,
            namespace: "common".to_owned(),
            key: "welcome".to_owned(),
            locale_code: "en-US".to_owned(),
            value: "Hello".to_owned(),
            status: TranslationStatus::Translated,
            description: None,
        })
        .unwrap();

    let collab_a = CollaborationService::new(service_a.store_arc(), "peer-a");
    collab_a.bootstrap(project.id).unwrap();

    service_a.store().backup_to(&b_db).unwrap();
    // Give peer B a distinct identity while keeping the same project/document.
    let conn = rusqlite::Connection::open(&b_db).unwrap();
    conn.execute("DELETE FROM collab_updates", []).unwrap();
    conn.execute("DELETE FROM collab_snapshots", []).unwrap();
    conn.execute("DELETE FROM collab_checkpoints", []).unwrap();
    conn.execute("DELETE FROM collab_documents", []).unwrap();
    conn.execute("DELETE FROM collab_peers", []).unwrap();
    drop(conn);

    let service_b = service(&b_db);
    let collab_b = CollaborationService::new(service_b.store_arc(), "peer-b");
    collab_b.bootstrap(project.id).unwrap();

    // Offline edits.
    collab_a
        .set_translation(
            project.id,
            "common",
            "welcome",
            "en-US",
            "Hola desde A",
            TranslationStatus::Translated,
        )
        .unwrap();
    collab_a
        .set_translation(
            project.id,
            "common",
            "save",
            "en-US",
            "Guardar A",
            TranslationStatus::Translated,
        )
        .unwrap();
    collab_b
        .set_translation(
            project.id,
            "common",
            "welcome",
            "en-US",
            "Bonjour depuis B",
            TranslationStatus::Translated,
        )
        .unwrap();
    collab_b
        .set_translation(
            project.id,
            "common",
            "cancel",
            "en-US",
            "Cancelar B",
            TranslationStatus::Translated,
        )
        .unwrap();

    let envelope_a = collab_a.export_update(project.id).unwrap();
    let envelope_b = collab_b.export_update(project.id).unwrap();

    collab_a.import_update(project.id, &envelope_b).unwrap();
    collab_b.import_update(project.id, &envelope_a).unwrap();

    // Convergence.
    assert_eq!(
        collab_a.open_document(project.id).unwrap().1.translations(),
        collab_b.open_document(project.id).unwrap().1.translations()
    );
    // Both peers received the distinct keys.
    let a_doc = collab_a.open_document(project.id).unwrap().1;
    assert!(a_doc.get_translation("common", "save", "en-US").is_some());
    assert!(a_doc.get_translation("common", "cancel", "en-US").is_some());

    // Re-import is idempotent.
    collab_a.import_update(project.id, &envelope_b).unwrap();
    collab_b.import_update(project.id, &envelope_a).unwrap();
    assert_eq!(
        collab_a.open_document(project.id).unwrap().1.translations(),
        collab_b.open_document(project.id).unwrap().1.translations()
    );

    // Restart both processes and verify persistence.
    drop(service_a);
    drop(service_b);
    let service_a2 = service(&a_db);
    let service_b2 = service(&b_db);
    let collab_a2 = CollaborationService::new(service_a2.store_arc(), "peer-a");
    let collab_b2 = CollaborationService::new(service_b2.store_arc(), "peer-b");
    assert_eq!(
        collab_a2
            .open_document(project.id)
            .unwrap()
            .1
            .translations(),
        collab_b2
            .open_document(project.id)
            .unwrap()
            .1
            .translations()
    );

    // SQL projections remain coherent.
    let entries_a = service_a2.list_project_translations(project.id).unwrap();
    let entries_b = service_b2.list_project_translations(project.id).unwrap();
    assert!(!entries_a.is_empty());
    assert_eq!(entries_a.len(), entries_b.len());
}

#[test]
fn rejects_tampered_envelope() {
    use slimlocalize_collaboration::SyncEnvelope;
    let envelope = SyncEnvelope::from_update(
        uuid::Uuid::new_v4(),
        "project:test".to_owned(),
        1,
        vec![1, 2, 3],
        vec![4, 5, 6],
    );
    let mut tampered = envelope.clone();
    tampered.payload =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"changed");
    assert!(tampered.validate(envelope.project_id).is_err());
}
