use std::fs;
use std::path::Path;

use slimlocalize_core::{
    AddLocaleInput, ConfigVisibility, CreateNamespaceInput, CreateProjectInput,
    CreateWorkspaceInput, DomainError, ExportOptions, ImportOptions, LocalizeService,
    SaveTranslationInput, SetConfigVariableInput, Store, TranslationStatus,
};
use slimlocalize_storage::SqliteStore;
use tempfile::TempDir;

fn service_at(path: &Path) -> LocalizeService<SqliteStore> {
    LocalizeService::new(SqliteStore::open(path).expect("open store"))
}

fn fixture_locales() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/locales")
}

struct Fixture {
    _dir: TempDir,
    db: std::path::PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("data/slimlocalize.sqlite3");
    Fixture { _dir: dir, db }
}

#[test]
fn migrations_are_versioned_and_idempotent() {
    let fixture = fixture();
    let store = SqliteStore::open(&fixture.db).expect("open");
    assert_eq!(store.schema_version().unwrap(), 1);
    store.health_check().unwrap();
    drop(store);
    let store = SqliteStore::open(&fixture.db).expect("reopen");
    assert_eq!(store.schema_version().unwrap(), 1);
    let stats = store.stats().unwrap();
    assert_eq!(stats.workspaces, 0);
    assert_eq!(stats.journal_mode.to_lowercase(), "wal");
}

#[test]
fn full_f0_f1_flow_persists_across_reopen() {
    let fixture = fixture();
    let service = service_at(&fixture.db);

    let workspace = service
        .create_workspace(CreateWorkspaceInput {
            name: "Demo Team".to_owned(),
            slug: None,
            description: Some("Local workspace".to_owned()),
        })
        .unwrap();
    assert_eq!(workspace.slug, "demo-team");

    let project = service
        .create_project(
            workspace.id,
            CreateProjectInput {
                name: "Demo App".to_owned(),
                slug: None,
                description: None,
                default_locale: None,
            },
        )
        .unwrap();

    let en = service
        .add_locale(
            project.id,
            AddLocaleInput {
                code: "en-us".to_owned(),
                name: Some("English (US)".to_owned()),
                is_default: false,
            },
        )
        .unwrap();
    assert!(en.is_default);
    assert_eq!(en.code, "en-US");

    let es = service
        .add_locale(
            project.id,
            AddLocaleInput {
                code: "es-PE".to_owned(),
                name: None,
                is_default: false,
            },
        )
        .unwrap();
    assert!(!es.is_default);

    let auth = service
        .create_namespace(
            project.id,
            CreateNamespaceInput {
                name: "auth".to_owned(),
            },
        )
        .unwrap();
    assert_eq!(auth.name, "auth");

    service
        .save_translation(SaveTranslationInput {
            project_id: project.id,
            namespace: "common".to_owned(),
            key: "welcome".to_owned(),
            locale_code: "en-US".to_owned(),
            value: "Welcome, {name}!".to_owned(),
            status: TranslationStatus::Translated,
            description: Some("Homepage greeting".to_owned()),
        })
        .unwrap();
    service
        .save_translation(SaveTranslationInput {
            project_id: project.id,
            namespace: "common".to_owned(),
            key: "welcome".to_owned(),
            locale_code: "es-PE".to_owned(),
            value: "¡Bienvenido!".to_owned(),
            status: TranslationStatus::Draft,
            description: None,
        })
        .unwrap();
    service
        .save_translation(SaveTranslationInput {
            project_id: project.id,
            namespace: "auth".to_owned(),
            key: "signIn".to_owned(),
            locale_code: "en-US".to_owned(),
            value: "Sign in".to_owned(),
            status: TranslationStatus::Reviewed,
            description: None,
        })
        .unwrap();

    let entries = service
        .list_translations(project.id, "es-PE", Some("common"))
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].value.as_deref(), Some("¡Bienvenido!"));
    assert_eq!(entries[0].status, Some(TranslationStatus::Draft));

    let entries = service
        .list_translations(project.id, "es-PE", Some("auth"))
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].value.is_none());
    assert!(entries[0].status.is_none());

    let environments = service.list_environments(project.id).unwrap();
    assert_eq!(environments.len(), 3);
    let development = environments
        .iter()
        .find(|environment| environment.name == "development")
        .unwrap();
    service
        .set_config_variable(SetConfigVariableInput {
            environment_id: development.id,
            key: "PUBLIC_API_URL".to_owned(),
            value: "http://localhost:3000".to_owned(),
            visibility: ConfigVisibility::Public,
        })
        .unwrap();
    let variables = service.list_config_variables(development.id).unwrap();
    assert_eq!(variables.len(), 1);
    assert_eq!(variables[0].visibility, ConfigVisibility::Public);

    let check = service.check_project(project.id).unwrap();
    assert_eq!(check.default_locale.as_deref(), Some("en-US"));
    assert_eq!(check.total_keys, 2);
    assert_eq!(check.missing.len(), 1);
    assert_eq!(check.missing[0].key, "signIn");
    assert_eq!(check.placeholder_mismatches.len(), 1);
    assert_eq!(check.placeholder_mismatches[0].key, "welcome");
    assert!(check.has_issues());

    let audit = service.list_audit_events(workspace.id, 100).unwrap();
    assert!(
        audit
            .iter()
            .any(|event| event.action == "translation.saved")
    );
    assert!(
        audit
            .iter()
            .any(|event| event.action == "config_variable.set")
    );

    // Stop the service entirely and reopen the database file.
    drop(service);
    let service = service_at(&fixture.db);
    let workspaces = service.list_workspaces().unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].slug, "demo-team");
    let projects = service.list_projects(workspaces[0].id).unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].default_locale.as_deref(), Some("en-US"));
    let entries = service
        .list_translations(projects[0].id, "en-US", Some("common"))
        .unwrap();
    assert_eq!(entries[0].value.as_deref(), Some("Welcome, {name}!"));
    let environments = service.list_environments(projects[0].id).unwrap();
    let development = environments
        .iter()
        .find(|environment| environment.name == "development")
        .unwrap();
    let variables = service.list_config_variables(development.id).unwrap();
    assert_eq!(variables[0].value, "http://localhost:3000");
}

#[test]
fn import_export_roundtrip_is_deterministic() {
    let fixture = fixture();
    let service = service_at(&fixture.db);
    let workspace = service
        .create_workspace(CreateWorkspaceInput {
            name: "Import".to_owned(),
            slug: None,
            description: None,
        })
        .unwrap();
    let project = service
        .create_project(
            workspace.id,
            CreateProjectInput {
                name: "Fixture".to_owned(),
                slug: None,
                description: None,
                default_locale: Some("en-US".to_owned()),
            },
        )
        .unwrap();

    let report = service
        .import_translations(
            project.id,
            ImportOptions {
                path: fixture_locales(),
                namespace: "common".to_owned(),
            },
        )
        .unwrap();
    assert_eq!(report.files_processed, 3);
    assert_eq!(report.locales_created, 2); // en-US was created with the project
    assert_eq!(report.keys_created, 5);
    assert_eq!(report.values_written, 14);
    assert!(report.skipped_files.is_empty());

    let check = service.check_project(project.id).unwrap();
    assert_eq!(check.default_locale.as_deref(), Some("en-US"));
    assert_eq!(check.total_keys, 5);
    assert!(
        check
            .missing
            .iter()
            .any(|missing| { missing.locale_code == "es-PE" && missing.key == "auth.signOut" })
    );
    assert!(
        check
            .placeholder_mismatches
            .iter()
            .any(|mismatch| mismatch.locale_code == "es-PE" && mismatch.key == "welcome")
    );

    let output_one = fixture._dir.path().join("export-one");
    let output_two = fixture._dir.path().join("export-two");
    let first = service
        .export_translations(
            project.id,
            ExportOptions {
                output_dir: output_one.clone(),
                format: Default::default(),
                nested: false,
                namespace: None,
            },
        )
        .unwrap();
    let second = service
        .export_translations(
            project.id,
            ExportOptions {
                output_dir: output_two.clone(),
                format: Default::default(),
                nested: false,
                namespace: None,
            },
        )
        .unwrap();
    assert_eq!(first.files.len(), 3);
    assert_eq!(first.total_keys, 14);
    assert_eq!(first.files, second.files);
    for file in &first.files {
        let left = fs::read(output_one.join(&file.relative_path)).unwrap();
        let right = fs::read(output_two.join(&file.relative_path)).unwrap();
        assert_eq!(left, right, "export must be deterministic");
        let text = String::from_utf8(left).unwrap();
        assert!(text.ends_with('\n'));
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(json.is_object());
        assert!(json.get("auth.signIn").is_some());
    }

    let nested_dir = fixture._dir.path().join("export-nested");
    service
        .export_translations(
            project.id,
            ExportOptions {
                output_dir: nested_dir.clone(),
                format: Default::default(),
                nested: true,
                namespace: None,
            },
        )
        .unwrap();
    let nested: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(nested_dir.join("en-US.json")).unwrap()).unwrap();
    assert_eq!(nested["auth"]["signIn"], "Sign in");

    // Re-import the deterministic export into a brand-new project.
    drop(service);
    let service = service_at(&fixture.db);
    let workspace = service
        .create_workspace(CreateWorkspaceInput {
            name: "Roundtrip".to_owned(),
            slug: None,
            description: None,
        })
        .unwrap();
    let roundtrip = service
        .create_project(
            workspace.id,
            CreateProjectInput {
                name: "Roundtrip".to_owned(),
                slug: None,
                description: None,
                default_locale: Some("en-US".to_owned()),
            },
        )
        .unwrap();
    let report = service
        .import_translations(
            roundtrip.id,
            ImportOptions {
                path: nested_dir,
                namespace: "common".to_owned(),
            },
        )
        .unwrap();
    assert_eq!(report.values_written, 14);
    let check = service.check_project(roundtrip.id).unwrap();
    assert_eq!(check.total_keys, 5);
}

#[test]
fn rejects_secret_variables_and_duplicate_resources() {
    let fixture = fixture();
    let service = service_at(&fixture.db);
    let workspace = service
        .create_workspace(CreateWorkspaceInput {
            name: "Security".to_owned(),
            slug: None,
            description: None,
        })
        .unwrap();
    assert!(matches!(
        service.create_workspace(CreateWorkspaceInput {
            name: "Security".to_owned(),
            slug: None,
            description: None,
        }),
        Err(DomainError::Conflict { .. })
    ));
    let project = service
        .create_project(
            workspace.id,
            CreateProjectInput {
                name: "App".to_owned(),
                slug: None,
                description: None,
                default_locale: None,
            },
        )
        .unwrap();
    let environment = service.list_environments(project.id).unwrap()[0].clone();
    let secret = service.set_config_variable(SetConfigVariableInput {
        environment_id: environment.id,
        key: "DATABASE_URL".to_owned(),
        value: "postgres://secret".to_owned(),
        visibility: ConfigVisibility::Public,
    });
    assert!(secret.is_ok(), "public variable is allowed in F1");

    assert!(matches!(
        ConfigVisibility::parse("secret"),
        Err(DomainError::Unsupported { .. })
    ));

    assert!(matches!(
        service.add_locale(
            project.id,
            AddLocaleInput {
                code: "../../etc".to_owned(),
                name: None,
                is_default: false,
            },
        ),
        Err(DomainError::Validation { .. })
    ));
    let first = service
        .add_locale(
            project.id,
            AddLocaleInput {
                code: "en-US".to_owned(),
                name: None,
                is_default: false,
            },
        )
        .unwrap();
    assert!(first.is_default);
    assert!(matches!(
        service.add_locale(
            project.id,
            AddLocaleInput {
                code: "en-US".to_owned(),
                name: None,
                is_default: false,
            },
        ),
        Err(DomainError::Conflict { .. })
    ));
}

#[test]
fn sql_injection_payloads_are_stored_as_data() {
    let fixture = fixture();
    let service = service_at(&fixture.db);
    let payload = "Robert'); DROP TABLE workspaces;--";
    let workspace = service
        .create_workspace(CreateWorkspaceInput {
            name: payload.to_owned(),
            slug: Some("injection-test".to_owned()),
            description: Some(payload.to_owned()),
        })
        .unwrap();
    assert_eq!(workspace.name, payload);
    let workspaces = service.list_workspaces().unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].name, payload);
}

#[test]
fn backup_creates_an_openable_database() {
    let fixture = fixture();
    let service = service_at(&fixture.db);
    service
        .create_workspace(CreateWorkspaceInput {
            name: "Backup".to_owned(),
            slug: None,
            description: None,
        })
        .unwrap();
    let backup_path = fixture._dir.path().join("backups/copy.sqlite3");
    let backup = service.store().backup_to(&backup_path).unwrap();
    assert!(backup.exists());
    let backup_service = service_at(&backup);
    assert_eq!(backup_service.list_workspaces().unwrap().len(), 1);
    // Refuses to overwrite.
    assert!(matches!(
        service.store().backup_to(&backup_path),
        Err(DomainError::Conflict { .. })
    ));
}

#[test]
fn import_rejects_arrays_and_invalid_json() {
    let fixture = fixture();
    let service = service_at(&fixture.db);
    let workspace = service
        .create_workspace(CreateWorkspaceInput {
            name: "Bad import".to_owned(),
            slug: None,
            description: None,
        })
        .unwrap();
    let project = service
        .create_project(
            workspace.id,
            CreateProjectInput {
                name: "App".to_owned(),
                slug: None,
                description: None,
                default_locale: None,
            },
        )
        .unwrap();
    let dir = fixture._dir.path().join("bad");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("en-US.json"), r#"{"array": ["a"]}"#).unwrap();
    let error = service
        .import_translations(
            project.id,
            ImportOptions {
                path: dir.clone(),
                namespace: "common".to_owned(),
            },
        )
        .unwrap_err();
    assert!(matches!(error, DomainError::Validation { .. }));

    fs::write(dir.join("en-US.json"), "{invalid").unwrap();
    let error = service
        .import_translations(
            project.id,
            ImportOptions {
                path: dir,
                namespace: "common".to_owned(),
            },
        )
        .unwrap_err();
    assert!(matches!(error, DomainError::Json { .. }));
}

#[cfg(unix)]
#[test]
fn import_rejects_symlink_escape() {
    use std::os::unix::fs::symlink;

    let fixture = fixture();
    let service = service_at(&fixture.db);
    let workspace = service
        .create_workspace(CreateWorkspaceInput {
            name: "Symlink".to_owned(),
            slug: None,
            description: None,
        })
        .unwrap();
    let project = service
        .create_project(
            workspace.id,
            CreateProjectInput {
                name: "App".to_owned(),
                slug: None,
                description: None,
                default_locale: None,
            },
        )
        .unwrap();
    let outside = fixture._dir.path().join("outside.json");
    fs::write(&outside, r#"{"welcome": "outside"}"#).unwrap();
    let dir = fixture._dir.path().join("imports");
    fs::create_dir_all(&dir).unwrap();
    symlink(&outside, dir.join("en-US.json")).unwrap();
    let error = service
        .import_translations(
            project.id,
            ImportOptions {
                path: dir,
                namespace: "common".to_owned(),
            },
        )
        .unwrap_err();
    assert!(matches!(error, DomainError::PathTraversal { .. }));
}

#[test]
fn empty_project_check_is_stable() {
    let fixture = fixture();
    let service = service_at(&fixture.db);
    let workspace = service
        .create_workspace(CreateWorkspaceInput {
            name: "Empty".to_owned(),
            slug: None,
            description: None,
        })
        .unwrap();
    let project = service
        .create_project(
            workspace.id,
            CreateProjectInput {
                name: "Empty App".to_owned(),
                slug: None,
                description: None,
                default_locale: None,
            },
        )
        .unwrap();
    let check = service.check_project(project.id).unwrap();
    assert!(check.default_locale.is_none());
    assert_eq!(check.total_keys, 0);
    assert!(!check.has_issues());
}
