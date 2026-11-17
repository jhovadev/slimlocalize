use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use serde_json::{Map as JsonMap, Value as JsonValue};
use uuid::Uuid;

use crate::error::{DomainError, Result};
use crate::model::{
    AddLocaleInput, AppInfo, AuditEvent, CheckReport, ConfigVariable, CreateEnvironmentInput,
    CreateNamespaceInput, CreateProjectInput, CreateWorkspaceInput, Environment, EnvironmentKind,
    ExportFile, ExportFormat, ExportOptions, ExportReport, ImportEntry, ImportOptions,
    ImportReport, KeySummary, Locale, MissingTranslation, Namespace, PlaceholderMismatch, Project,
    ProjectSnapshot, SaveTranslationInput, SetConfigVariableInput, TranslationEntry,
    TranslationStatus, Workspace,
};
use crate::paths;
use crate::ports::Store;
use crate::validation;

const MAX_IMPORT_FILES: usize = 500;
const MAX_IMPORT_JSON_BYTES: u64 = 8 * 1024 * 1024;

/// Application service shared by CLI, Tauri, HTTP and MCP adapters.
///
/// The service validates input, applies domain rules and delegates persistence
/// to a [`Store`]. It contains no SQL, HTTP or UI concerns.
pub struct LocalizeService<S: Store> {
    store: Arc<S>,
}

impl<S: Store> std::fmt::Debug for LocalizeService<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalizeService").finish_non_exhaustive()
    }
}

impl<S: Store> LocalizeService<S> {
    #[must_use]
    pub fn new(store: S) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    #[must_use]
    pub fn with_shared_store(store: Arc<S>) -> Self {
        Self { store }
    }

    #[must_use]
    pub fn store(&self) -> &S {
        &self.store
    }

    #[must_use]
    pub fn store_arc(&self) -> Arc<S> {
        Arc::clone(&self.store)
    }

    // ---------------------------------------------------------------------
    // Workspaces
    // ---------------------------------------------------------------------

    pub fn create_workspace(&self, input: CreateWorkspaceInput) -> Result<Workspace> {
        let name = input.name.trim().to_owned();
        validation::validate_name("workspace name", &name)?;
        let slug = input.slug.unwrap_or_else(|| validation::slugify(&name));
        validation::validate_slug(&slug)?;
        if self.store.find_workspace_by_slug(&slug)?.is_some() {
            return Err(DomainError::conflict(
                "workspace",
                "slug",
                format!("already exists: {slug}"),
            ));
        }
        let at = Utc::now();
        let workspace = Workspace::new(
            Uuid::new_v4(),
            slug,
            name,
            input.description.map(|value| value.trim().to_owned()),
            at,
        );
        self.store.insert_workspace(&workspace)?;
        self.audit(
            Some(workspace.id),
            None,
            "workspace.created",
            "workspace",
            Some(workspace.id),
            serde_json::json!({ "slug": workspace.slug }),
        );
        Ok(workspace)
    }

    pub fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        self.store.list_workspaces()
    }

    pub fn get_workspace(&self, id: Uuid) -> Result<Workspace> {
        self.store
            .find_workspace_by_id(id)?
            .ok_or_else(|| DomainError::not_found("workspace", id.to_string()))
    }

    /// Resolve by UUID (when parseable) or slug.
    pub fn resolve_workspace(&self, reference: &str) -> Result<Workspace> {
        let reference = reference.trim();
        if let Ok(id) = Uuid::parse_str(reference)
            && let Some(workspace) = self.store.find_workspace_by_id(id)?
        {
            return Ok(workspace);
        }
        self.store
            .find_workspace_by_slug(reference)?
            .ok_or_else(|| DomainError::not_found("workspace", reference))
    }

    pub fn list_audit_events(&self, workspace_id: Uuid, limit: usize) -> Result<Vec<AuditEvent>> {
        self.store.list_audit_events(workspace_id, limit)
    }

    // ---------------------------------------------------------------------
    // Projects
    // ---------------------------------------------------------------------

    pub fn create_project(&self, workspace_id: Uuid, input: CreateProjectInput) -> Result<Project> {
        let workspace = self.get_workspace(workspace_id)?;
        let name = input.name.trim().to_owned();
        validation::validate_name("project name", &name)?;
        let slug = input.slug.unwrap_or_else(|| validation::slugify(&name));
        validation::validate_slug(&slug)?;
        if self
            .store
            .find_project_by_slug(workspace_id, &slug)?
            .is_some()
        {
            return Err(DomainError::conflict(
                "project",
                "slug",
                format!("already exists in workspace: {slug}"),
            ));
        }

        let at = Utc::now();
        let normalized_default = input
            .default_locale
            .as_deref()
            .map(validation::normalize_locale_code)
            .transpose()?;
        let mut project = Project::new(
            Uuid::new_v4(),
            workspace.id,
            slug,
            name,
            input.description.map(|value| value.trim().to_owned()),
            normalized_default.clone(),
            at,
        );

        let default_locale = normalized_default
            .map(|code| Locale::new(Uuid::new_v4(), project.id, code.clone(), code, true, at));

        let environments = ["development", "staging", "production"]
            .into_iter()
            .zip([
                EnvironmentKind::Development,
                EnvironmentKind::Staging,
                EnvironmentKind::Production,
            ])
            .map(|(name, kind)| {
                Environment::new(Uuid::new_v4(), project.id, name.to_owned(), kind, at)
            })
            .collect::<Vec<_>>();

        self.store.insert_project_with_defaults(
            &project,
            default_locale.as_ref(),
            &environments,
        )?;
        // Keep the in-memory object in sync with the storage-generated defaults.
        if project.default_locale.is_none() {
            project.default_locale = default_locale.as_ref().map(|locale| locale.code.clone());
        }
        self.audit(
            Some(workspace.id),
            Some(project.id),
            "project.created",
            "project",
            Some(project.id),
            serde_json::json!({ "slug": project.slug }),
        );
        Ok(project)
    }

    pub fn list_projects(&self, workspace_id: Uuid) -> Result<Vec<Project>> {
        self.get_workspace(workspace_id)?;
        self.store.list_projects(workspace_id)
    }

    pub fn get_project(&self, id: Uuid) -> Result<Project> {
        self.store
            .find_project_by_id(id)?
            .ok_or_else(|| DomainError::not_found("project", id.to_string()))
    }

    pub fn resolve_project(&self, workspace_id: Uuid, reference: &str) -> Result<Project> {
        let reference = reference.trim();
        if let Ok(id) = Uuid::parse_str(reference)
            && let Some(project) = self.store.find_project_by_id(id)?
            && project.workspace_id == workspace_id
        {
            return Ok(project);
        }
        self.store
            .find_project_by_slug(workspace_id, reference)?
            .ok_or_else(|| DomainError::not_found("project", reference))
    }

    pub fn project_snapshot(&self, project_id: Uuid) -> Result<ProjectSnapshot> {
        let project = self.get_project(project_id)?;
        Ok(ProjectSnapshot {
            locales: self.store.list_locales(project_id)?,
            namespaces: self.store.list_namespaces(project_id)?,
            environments: self.store.list_environments(project_id)?,
            project,
        })
    }

    // ---------------------------------------------------------------------
    // Locales & namespaces
    // ---------------------------------------------------------------------

    pub fn add_locale(&self, project_id: Uuid, input: AddLocaleInput) -> Result<Locale> {
        let project = self.get_project(project_id)?;
        let code = validation::normalize_locale_code(&input.code)?;
        if self.store.find_locale_by_code(project_id, &code)?.is_some() {
            return Err(DomainError::conflict(
                "locale",
                "code",
                format!("already exists in project: {code}"),
            ));
        }
        let existing = self.store.list_locales(project_id)?;
        let is_default = input.is_default || existing.is_empty();
        let name = input
            .name
            .map_or_else(|| code.clone(), |value| value.trim().to_owned());
        validation::validate_name("locale name", &name)?;
        let locale = Locale::new(
            Uuid::new_v4(),
            project.id,
            code,
            name,
            is_default,
            Utc::now(),
        );
        self.store.insert_locale(&locale)?;
        self.audit(
            Some(project.workspace_id),
            Some(project.id),
            "locale.added",
            "locale",
            Some(locale.id),
            serde_json::json!({ "code": locale.code, "isDefault": is_default }),
        );
        Ok(locale)
    }

    pub fn list_locales(&self, project_id: Uuid) -> Result<Vec<Locale>> {
        self.get_project(project_id)?;
        self.store.list_locales(project_id)
    }

    pub fn set_default_locale(&self, project_id: Uuid, code: &str) -> Result<Locale> {
        let project = self.get_project(project_id)?;
        let code = validation::normalize_locale_code(code)?;
        let locale = self
            .store
            .find_locale_by_code(project_id, &code)?
            .ok_or_else(|| DomainError::not_found("locale", code.clone()))?;
        self.store.set_default_locale(project_id, locale.id)?;
        self.audit(
            Some(project.workspace_id),
            Some(project.id),
            "locale.default_changed",
            "locale",
            Some(locale.id),
            serde_json::json!({ "code": code }),
        );
        Ok(Locale {
            is_default: true,
            ..locale
        })
    }

    pub fn create_namespace(
        &self,
        project_id: Uuid,
        input: CreateNamespaceInput,
    ) -> Result<Namespace> {
        let project = self.get_project(project_id)?;
        let name = input.name.trim().to_owned();
        validation::validate_namespace(&name)?;
        if self
            .store
            .find_namespace_by_name(project_id, &name)?
            .is_some()
        {
            return Err(DomainError::conflict(
                "namespace",
                "name",
                format!("already exists in project: {name}"),
            ));
        }
        let namespace = Namespace::new(Uuid::new_v4(), project.id, name, Utc::now());
        self.store.insert_namespace(&namespace)?;
        self.audit(
            Some(project.workspace_id),
            Some(project.id),
            "namespace.created",
            "namespace",
            Some(namespace.id),
            serde_json::json!({ "name": namespace.name }),
        );
        Ok(namespace)
    }

    pub fn list_namespaces(&self, project_id: Uuid) -> Result<Vec<Namespace>> {
        self.get_project(project_id)?;
        self.store.list_namespaces(project_id)
    }

    // ---------------------------------------------------------------------
    // Translations
    // ---------------------------------------------------------------------

    pub fn save_translation(&self, input: SaveTranslationInput) -> Result<TranslationEntry> {
        let project = self.get_project(input.project_id)?;
        let locale_code = validation::normalize_locale_code(&input.locale_code)?;
        validation::validate_namespace(&input.namespace)?;
        validation::validate_key(&input.key)?;
        validation::validate_translation_value(&input.value)?;
        let locale = self
            .store
            .find_locale_by_code(project.id, &locale_code)?
            .ok_or_else(|| {
                DomainError::not_found(
                    "locale",
                    format!("{locale_code} (add the locale to the project first)"),
                )
            })?;

        let normalized = SaveTranslationInput {
            project_id: project.id,
            namespace: input.namespace.trim().to_owned(),
            key: input.key.trim().to_owned(),
            locale_code: locale.code.clone(),
            value: input.value,
            status: input.status,
            description: input.description.map(|value| value.trim().to_owned()),
        };
        let entry = self.store.save_translation(&normalized)?;
        self.audit(
            Some(project.workspace_id),
            Some(project.id),
            "translation.saved",
            "translation",
            Some(entry.key_id),
            serde_json::json!({
                "key": entry.key,
                "locale": entry.locale_code,
                "namespace": entry.namespace,
                "status": entry.status.map(TranslationStatus::as_str),
            }),
        );
        Ok(entry)
    }

    pub fn list_translations(
        &self,
        project_id: Uuid,
        locale_code: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<TranslationEntry>> {
        self.get_project(project_id)?;
        let code = validation::normalize_locale_code(locale_code)?;
        if self.store.find_locale_by_code(project_id, &code)?.is_none() {
            return Err(DomainError::not_found("locale", code));
        }
        if let Some(namespace) = namespace {
            validation::validate_namespace(namespace)?;
        }
        self.store.list_translations(project_id, &code, namespace)
    }

    pub fn list_keys(&self, project_id: Uuid) -> Result<Vec<KeySummary>> {
        self.get_project(project_id)?;
        self.store.list_keys(project_id)
    }

    pub fn get_translation(
        &self,
        project_id: Uuid,
        locale_code: &str,
        namespace: &str,
        key: &str,
    ) -> Result<Option<TranslationEntry>> {
        validation::validate_key(key)?;
        let entries = self.list_translations(project_id, locale_code, Some(namespace))?;
        Ok(entries.into_iter().find(|entry| entry.key == key))
    }

    pub fn search_translations(
        &self,
        project_id: Uuid,
        query: &str,
        locale_code: Option<&str>,
        limit: usize,
    ) -> Result<Vec<TranslationEntry>> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Err(DomainError::validation("query", "must not be empty"));
        }
        let limit = limit.clamp(1, 500);
        let locales = self.store.list_locales(project_id)?;
        let codes = match locale_code {
            Some(code) => vec![validation::normalize_locale_code(code)?],
            None => locales.into_iter().map(|locale| locale.code).collect(),
        };
        let mut matches = Vec::new();
        for code in codes {
            let entries = self.store.list_translations(project_id, &code, None)?;
            for entry in entries {
                let key_match = entry.key.to_lowercase().contains(&query);
                let value_match = entry
                    .value
                    .as_deref()
                    .is_some_and(|value| value.to_lowercase().contains(&query));
                if key_match || value_match {
                    matches.push(entry);
                    if matches.len() >= limit {
                        return Ok(matches);
                    }
                }
            }
        }
        Ok(matches)
    }

    pub fn find_missing_translations(&self, project_id: Uuid) -> Result<Vec<MissingTranslation>> {
        Ok(self.check_project(project_id)?.missing)
    }

    pub fn check_project(&self, project_id: Uuid) -> Result<CheckReport> {
        let project = self.get_project(project_id)?;
        let locales = self.store.list_locales(project_id)?;
        let keys = self.store.list_keys(project_id)?;
        let default_locale = project.default_locale.clone().or_else(|| {
            locales
                .iter()
                .find(|locale| locale.is_default)
                .map(|locale| locale.code.clone())
        });

        let locale_codes: Vec<String> = locales.iter().map(|locale| locale.code.clone()).collect();
        let Some(default_code) = default_locale.clone() else {
            return Ok(CheckReport {
                project_id: project.id,
                default_locale: None,
                locales: locale_codes,
                total_keys: keys.len(),
                checked_values: 0,
                missing: Vec::new(),
                placeholder_mismatches: Vec::new(),
            });
        };

        let mut values_by_locale: HashMap<String, HashMap<(String, String), Option<String>>> =
            HashMap::new();
        for locale in &locales {
            let entries = self
                .store
                .list_translations(project_id, &locale.code, None)?;
            let mut map = HashMap::new();
            for entry in entries {
                map.insert(
                    (entry.namespace, entry.key),
                    entry.value.filter(|value| !value.trim().is_empty()),
                );
            }
            values_by_locale.insert(locale.code.clone(), map);
        }

        let source_values = values_by_locale
            .get(&default_code)
            .cloned()
            .unwrap_or_default();
        let mut missing = Vec::new();
        let mut placeholder_mismatches = Vec::new();
        let mut checked_values = 0usize;

        for locale in &locales {
            let Some(values) = values_by_locale.get(&locale.code) else {
                continue;
            };
            for key in &keys {
                let composite = (key.namespace.clone(), key.key.clone());
                let value = values.get(&composite).and_then(Option::as_deref);
                if let Some(value) = value {
                    checked_values += 1;
                    if locale.code == default_code {
                        continue;
                    }
                    if let Some(source) = source_values.get(&composite).and_then(Option::as_deref) {
                        let expected = validation::extract_placeholders(source)
                            .into_iter()
                            .collect::<Vec<_>>();
                        let actual = validation::extract_placeholders(value)
                            .into_iter()
                            .collect::<Vec<_>>();
                        if expected != actual {
                            placeholder_mismatches.push(PlaceholderMismatch {
                                locale_code: locale.code.clone(),
                                source_locale: default_code.clone(),
                                namespace: key.namespace.clone(),
                                key: key.key.clone(),
                                expected,
                                actual,
                            });
                        }
                    }
                } else if locale.code != default_code {
                    missing.push(MissingTranslation {
                        locale_code: locale.code.clone(),
                        namespace: key.namespace.clone(),
                        key: key.key.clone(),
                    });
                }
            }
        }

        Ok(CheckReport {
            project_id: project.id,
            default_locale,
            locales: locale_codes,
            total_keys: keys.len(),
            checked_values,
            missing,
            placeholder_mismatches,
        })
    }

    // ---------------------------------------------------------------------
    // Import / export
    // ---------------------------------------------------------------------

    pub fn import_translations(
        &self,
        project_id: Uuid,
        options: ImportOptions,
    ) -> Result<ImportReport> {
        let project = self.get_project(project_id)?;
        validation::validate_namespace(&options.namespace)?;
        let input_path = options.path.clone();
        let canonical = paths::canonicalize_existing(&input_path)?;
        let (files, skipped_files) = self.collect_import_files(&canonical)?;
        if files.is_empty() {
            return Err(DomainError::validation(
                "path",
                "no .json locale files found to import",
            ));
        }

        let mut entries: Vec<ImportEntry> = Vec::new();
        let mut seen: HashMap<String, HashSet<String>> = HashMap::new();
        for file in &files {
            let stem = file
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| {
                    DomainError::validation("path", "locale file name must be valid UTF-8")
                })?;
            let locale_code = validation::normalize_locale_code(stem).map_err(|error| {
                DomainError::validation(
                    "path",
                    format!(
                        "cannot derive a locale code from {}: {error}",
                        file.display()
                    ),
                )
            })?;
            let bytes = paths::read_limited_file(file, MAX_IMPORT_JSON_BYTES)?;
            let json: JsonValue =
                serde_json::from_slice(&bytes).map_err(|source| DomainError::Json {
                    context: file.display().to_string(),
                    source,
                })?;
            let object = json.as_object().ok_or_else(|| {
                DomainError::validation(
                    "import",
                    format!("{} must contain a JSON object", file.display()),
                )
            })?;
            let mut flat = BTreeMap::new();
            flatten_object(object, "", &mut flat)?;
            let locale_seen = seen.entry(locale_code.clone()).or_default();
            for (key, value) in flat {
                validation::validate_key(&key)?;
                if !locale_seen.insert(key.clone()) {
                    return Err(DomainError::conflict(
                        "translation key",
                        "key",
                        format!("duplicate key {key} for locale {locale_code}"),
                    ));
                }
                entries.push(ImportEntry {
                    locale_code: locale_code.clone(),
                    key,
                    value,
                });
            }
        }

        entries.sort_by(|a, b| {
            a.locale_code
                .cmp(&b.locale_code)
                .then_with(|| a.key.cmp(&b.key))
        });

        let summary =
            self.store
                .apply_import(project.id, &options.namespace, &entries, Utc::now())?;

        let report = ImportReport {
            path: input_path,
            namespace: options.namespace,
            files_processed: files.len(),
            locales_created: summary.locales_created,
            keys_created: summary.keys_created,
            values_written: summary.values_written,
            skipped_files,
        };
        self.audit(
            Some(project.workspace_id),
            Some(project.id),
            "translations.imported",
            "project",
            Some(project.id),
            serde_json::json!({
                "files": report.files_processed,
                "keysCreated": report.keys_created,
                "valuesWritten": report.values_written,
            }),
        );
        Ok(report)
    }

    pub fn export_translations(
        &self,
        project_id: Uuid,
        options: ExportOptions,
    ) -> Result<ExportReport> {
        let project = self.get_project(project_id)?;
        if options.format != ExportFormat::Json {
            return Err(DomainError::Unsupported {
                feature: "only JSON export is available in F1".to_owned(),
            });
        }
        if let Some(namespace) = options.namespace.as_deref() {
            validation::validate_namespace(namespace)?;
        }
        let locales = self.store.list_locales(project_id)?;
        let output_dir = paths::ensure_directory(&options.output_dir)?;
        let mut files = Vec::new();
        let mut total_keys = 0usize;

        for locale in &locales {
            let entries = self.store.list_translations(
                project_id,
                &locale.code,
                options.namespace.as_deref(),
            )?;
            let mut flat: BTreeMap<String, String> = BTreeMap::new();
            let mut key_origins: HashMap<String, String> = HashMap::new();
            for entry in entries {
                if let Some(value) = entry.value {
                    if let Some(origin) = key_origins.get(&entry.key)
                        && origin != &entry.namespace
                    {
                        return Err(DomainError::conflict(
                            "export",
                            "key",
                            format!(
                                "key {:?} exists in namespaces {origin:?} and {:?}; \
                                     use the namespace filter to export unambiguously",
                                entry.key, entry.namespace
                            ),
                        ));
                    }
                    key_origins.insert(entry.key.clone(), entry.namespace.clone());
                    flat.insert(entry.key, value);
                }
            }
            let json = if options.nested {
                nested_json(&flat)?
            } else {
                let mut object = JsonMap::new();
                for (key, value) in &flat {
                    object.insert(key.clone(), JsonValue::String(value.clone()));
                }
                JsonValue::Object(object)
            };
            let mut serialized =
                serde_json::to_string_pretty(&json).map_err(|source| DomainError::Json {
                    context: "export".to_owned(),
                    source,
                })?;
            serialized.push('\n');
            let filename = format!("{}.json", locale.code);
            let target = paths::safe_join(&output_dir, &filename)?;
            std::fs::write(&target, serialized).map_err(|source| DomainError::Io {
                context: format!("cannot write {}", target.display()),
                source,
            })?;
            total_keys += flat.len();
            files.push(ExportFile {
                locale_code: locale.code.clone(),
                relative_path: filename,
                key_count: flat.len(),
            });
        }

        let report = ExportReport {
            project_id: project.id,
            output_dir,
            files,
            total_keys,
        };
        self.audit(
            Some(project.workspace_id),
            Some(project.id),
            "translations.exported",
            "project",
            Some(project.id),
            serde_json::json!({
                "files": report.files.len(),
                "totalKeys": report.total_keys,
            }),
        );
        Ok(report)
    }

    fn collect_import_files(&self, canonical: &Path) -> Result<(Vec<PathBuf>, Vec<String>)> {
        if canonical.is_file() {
            let extension = canonical.extension().and_then(|value| value.to_str());
            if extension != Some("json") {
                return Err(DomainError::validation(
                    "path",
                    "import file must use the .json extension",
                ));
            }
            return Ok((vec![canonical.to_path_buf()], Vec::new()));
        }

        let mut files = Vec::new();
        let mut skipped = Vec::new();
        let read_dir = std::fs::read_dir(canonical).map_err(|source| DomainError::Io {
            context: format!("cannot read directory {}", canonical.display()),
            source,
        })?;
        for entry in read_dir {
            let entry = entry.map_err(|source| DomainError::Io {
                context: format!("cannot read directory entry in {}", canonical.display()),
                source,
            })?;
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();
            if path.is_file() {
                if path.extension().and_then(|value| value.to_str()) == Some("json") {
                    let resolved = paths::canonicalize_existing(&path)?;
                    if !resolved.starts_with(canonical) {
                        return Err(DomainError::PathTraversal { path });
                    }
                    files.push(resolved);
                    if files.len() > MAX_IMPORT_FILES {
                        return Err(DomainError::LimitExceeded {
                            context: "import file count".to_owned(),
                            limit: MAX_IMPORT_FILES as u64,
                        });
                    }
                } else {
                    skipped.push(file_name);
                }
            } else {
                skipped.push(file_name);
            }
        }
        files.sort();
        skipped.sort();
        Ok((files, skipped))
    }

    // ---------------------------------------------------------------------
    // Environments & configuration
    // ---------------------------------------------------------------------

    pub fn list_environments(&self, project_id: Uuid) -> Result<Vec<Environment>> {
        self.get_project(project_id)?;
        self.store.list_environments(project_id)
    }

    pub fn create_environment(
        &self,
        project_id: Uuid,
        input: CreateEnvironmentInput,
    ) -> Result<Environment> {
        let project = self.get_project(project_id)?;
        let name = input.name.trim().to_owned();
        validation::validate_namespace(&name)?;
        if self
            .store
            .find_environment_by_name(project_id, &name)?
            .is_some()
        {
            return Err(DomainError::conflict(
                "environment",
                "name",
                format!("already exists in project: {name}"),
            ));
        }
        let environment =
            Environment::new(Uuid::new_v4(), project.id, name, input.kind, Utc::now());
        self.store.insert_environment(&environment)?;
        self.audit(
            Some(project.workspace_id),
            Some(project.id),
            "environment.created",
            "environment",
            Some(environment.id),
            serde_json::json!({ "name": environment.name, "kind": environment.kind.as_str() }),
        );
        Ok(environment)
    }

    pub fn get_environment(&self, id: Uuid) -> Result<Environment> {
        self.store
            .find_environment_by_id(id)?
            .ok_or_else(|| DomainError::not_found("environment", id.to_string()))
    }

    pub fn set_config_variable(&self, input: SetConfigVariableInput) -> Result<ConfigVariable> {
        let environment = self.get_environment(input.environment_id)?;
        let project = self.get_project(environment.project_id)?;
        validation::validate_config_key(&input.key)?;
        validation::validate_translation_value(&input.value)?;
        let normalized = SetConfigVariableInput {
            environment_id: environment.id,
            key: input.key.trim().to_owned(),
            value: input.value,
            visibility: input.visibility,
        };
        let variable = self.store.upsert_config_variable(&normalized, Utc::now())?;
        self.audit(
            Some(project.workspace_id),
            Some(project.id),
            "config_variable.set",
            "config_variable",
            Some(variable.id),
            serde_json::json!({
                "key": variable.key,
                "environment": environment.name,
                "visibility": variable.visibility.as_str(),
            }),
        );
        Ok(variable)
    }

    pub fn list_config_variables(&self, environment_id: Uuid) -> Result<Vec<ConfigVariable>> {
        self.get_environment(environment_id)?;
        self.store.list_config_variables(environment_id)
    }

    // ---------------------------------------------------------------------
    // Diagnostics
    // ---------------------------------------------------------------------

    #[must_use]
    pub fn app_info(&self, platform: &str, database_path: &Path) -> AppInfo {
        AppInfo {
            name: "SlimLocalize".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            platform: platform.to_owned(),
            storage: "SQLite (local, WAL)".to_owned(),
            database_path: database_path.display().to_string(),
            local_first: true,
        }
    }

    fn audit(
        &self,
        workspace_id: Option<Uuid>,
        project_id: Option<Uuid>,
        action: &str,
        entity_type: &str,
        entity_id: Option<Uuid>,
        payload: JsonValue,
    ) {
        let event = AuditEvent::new(
            workspace_id,
            project_id,
            action,
            entity_type,
            entity_id,
            payload,
        );
        if let Err(error) = self.store.insert_audit_event(&event) {
            tracing::warn!(error = %error, action, "failed to persist audit event");
        }
    }
}

fn flatten_object(
    object: &JsonMap<String, JsonValue>,
    prefix: &str,
    out: &mut BTreeMap<String, String>,
) -> Result<()> {
    for (key, value) in object {
        if key.is_empty() {
            return Err(DomainError::validation(
                "import",
                "empty keys are not allowed",
            ));
        }
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match value {
            JsonValue::Object(nested) => flatten_object(nested, &path, out)?,
            JsonValue::String(text) => {
                out.insert(path, text.clone());
            }
            JsonValue::Array(_) => {
                return Err(DomainError::validation(
                    "import",
                    format!("arrays are not supported at key {path:?}"),
                ));
            }
            other => {
                return Err(DomainError::validation(
                    "import",
                    format!(
                        "value at key {path:?} must be a string, got {}",
                        json_type_name(other)
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn json_type_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "boolean",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

fn nested_json(flat: &BTreeMap<String, String>) -> Result<JsonValue> {
    let mut root = JsonMap::new();
    for (key, value) in flat {
        let parts: Vec<&str> = key.split('.').collect();
        let mut cursor = &mut root;
        for (index, part) in parts.iter().enumerate() {
            if part.is_empty() {
                return Err(DomainError::validation(
                    "export",
                    format!("invalid empty segment in key {key:?}"),
                ));
            }
            if index == parts.len() - 1 {
                if cursor.contains_key(*part) {
                    return Err(DomainError::conflict(
                        "export",
                        "key",
                        format!("nested key collision at {key:?}"),
                    ));
                }
                cursor.insert((*part).to_owned(), JsonValue::String(value.clone()));
            } else {
                let entry = cursor
                    .entry((*part).to_owned())
                    .or_insert_with(|| JsonValue::Object(JsonMap::new()));
                if !entry.is_object() {
                    return Err(DomainError::conflict(
                        "export",
                        "key",
                        format!("nested key collision at {key:?}"),
                    ));
                }
                cursor = entry.as_object_mut().expect("checked object");
            }
        }
    }
    Ok(JsonValue::Object(root))
}
