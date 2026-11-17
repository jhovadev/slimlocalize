use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use chrono::Local;
use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::json;
use slimlocalize_core::ports::Store;
use slimlocalize_core::{
    AddLocaleInput, ConfigVisibility, CreateEnvironmentInput, CreateNamespaceInput,
    CreateProjectInput, CreateWorkspaceInput, DomainError, Environment, EnvironmentKind,
    ExportFormat, ExportOptions, ImportOptions, LocalizeService, Project, SaveTranslationInput,
    SetConfigVariableInput, TranslationStatus, Workspace,
};
use slimlocalize_mcp::McpServer;
use slimlocalize_storage::{SqliteStore, StorageStats, default_database_path};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "localize",
    version,
    about = "Local-first i18n workspace. Cloud optional, SQLite local by default.",
    propagate_version = true
)]
struct Cli {
    /// SQLite database path. Defaults to the OS data directory.
    #[arg(long, global = true, env = "SLIMLOCALIZE_DB")]
    db: Option<PathBuf>,

    /// Machine-readable JSON on stdout.
    #[arg(long, global = true)]
    json: bool,

    /// Log level for stderr diagnostics.
    #[arg(long, global = true, env = "SLIMLOCALIZE_LOG", default_value = "warn")]
    log_level: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Check runtime, database and storage health.
    Doctor,
    /// Manage workspaces.
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    /// Manage projects.
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Manage project locales.
    Locale {
        #[command(subcommand)]
        command: LocaleCommand,
    },
    /// Manage namespaces.
    Namespace {
        #[command(subcommand)]
        command: NamespaceCommand,
    },
    /// Inspect translation keys.
    Keys {
        #[command(subcommand)]
        command: KeysCommand,
    },
    /// Read or write one translation.
    Translation {
        #[command(subcommand)]
        command: TranslationCommand,
    },
    /// Import JSON locale files.
    Import(ImportCommand),
    /// Export deterministic JSON locale files.
    Export(ExportCommand),
    /// Validate missing keys and placeholders. Exits 1 when issues exist.
    Check(CheckCommand),
    /// Manage environments.
    Env {
        #[command(subcommand)]
        command: EnvCommand,
    },
    /// Manage non-secret environment variables.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Database maintenance.
    Db {
        #[command(subcommand)]
        command: DbCommand,
    },
    /// Local read-only MCP server over stdio.
    Mcp {
        #[command(subcommand)]
        command: McpCommand,
    },
}

#[derive(Debug, Subcommand)]
enum WorkspaceCommand {
    Create {
        name: String,
        #[arg(long)]
        slug: Option<String>,
        #[arg(long)]
        description: Option<String>,
    },
    List,
    Show {
        workspace: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    Create {
        name: String,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        slug: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// First locale, for example en-US.
        #[arg(long)]
        default_locale: Option<String>,
    },
    List {
        #[arg(long)]
        workspace: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum LocaleCommand {
    Add {
        code: String,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        default: bool,
    },
    List {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        project: Option<String>,
    },
    SetDefault {
        code: String,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        project: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum NamespaceCommand {
    Create {
        name: String,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        project: Option<String>,
    },
    List {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        project: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum KeysCommand {
    List {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        locale: Option<String>,
        #[arg(long)]
        namespace: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum TranslationCommand {
    Set {
        key: String,
        value: String,
        #[arg(long)]
        locale: String,
        #[arg(long, default_value = "common")]
        namespace: String,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long, default_value = "translated")]
        status: String,
        #[arg(long)]
        description: Option<String>,
    },
    Get {
        key: String,
        #[arg(long)]
        locale: String,
        #[arg(long, default_value = "common")]
        namespace: String,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        project: Option<String>,
    },
}

#[derive(Debug, clap::Args)]
struct ImportCommand {
    path: PathBuf,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long)]
    project: Option<String>,
    #[arg(long, default_value = "common")]
    namespace: String,
}

#[derive(Debug, clap::Args)]
struct ExportCommand {
    #[arg(long, default_value = "json")]
    format: String,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long)]
    project: Option<String>,
    #[arg(long)]
    nested: bool,
    #[arg(long)]
    namespace: Option<String>,
}

#[derive(Debug, clap::Args)]
struct CheckCommand {
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long)]
    project: Option<String>,
}

#[derive(Debug, Subcommand)]
enum EnvCommand {
    List {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        project: Option<String>,
    },
    Create {
        name: String,
        #[arg(long, default_value = "custom")]
        kind: String,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        project: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    Set {
        key: String,
        value: String,
        #[arg(long)]
        environment: String,
        #[arg(long, default_value = "public")]
        visibility: String,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        project: Option<String>,
    },
    List {
        #[arg(long)]
        environment: String,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        project: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum DbCommand {
    /// Apply pending migrations (opening the DB also migrates it).
    Migrate,
    /// Show schema and row counts.
    Status,
    /// Create a consistent backup with VACUUM INTO.
    Backup {
        /// Destination file. Defaults to <data dir>/backups/slimlocalize-<time>.sqlite3.
        out: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum McpCommand {
    /// Serve the read-only tool registry over stdin/stdout.
    Serve,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(&cli.log_level);

    match run(&cli) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            if cli.json {
                println!(
                    "{}",
                    json!({
                        "error": {
                            "code": error.code(),
                            "message": error.to_string()
                        }
                    })
                );
            } else {
                eprintln!("error: {error}");
            }
            ExitCode::from(1)
        }
    }
}

fn run(cli: &Cli) -> Result<u8, CliError> {
    let db_path = cli.db.clone().unwrap_or_else(default_database_path);
    let store = SqliteStore::open(&db_path)?;
    let service = Arc::new(LocalizeService::new(store));
    let json = cli.json;

    match &cli.command {
        Command::Doctor => doctor(&service, &db_path, json)?,
        Command::Workspace { command } => workspace(&service, command, json)?,
        Command::Project { command } => project(&service, command, json)?,
        Command::Locale { command } => locale(&service, command, json)?,
        Command::Namespace { command } => namespace(&service, command, json)?,
        Command::Keys { command } => keys(&service, command, json)?,
        Command::Translation { command } => translation(&service, command, json)?,
        Command::Import(command) => import(&service, command, json)?,
        Command::Export(command) => export(&service, command, json)?,
        Command::Check(command) => return check(&service, command, json),
        Command::Env { command } => environment(&service, command, json)?,
        Command::Config { command } => config(&service, command, json)?,
        Command::Db { command } => database(&service, command, &db_path, json)?,
        Command::Mcp { command } => match command {
            McpCommand::Serve => {
                let server = McpServer::new(Arc::clone(&service));
                let stdin = io::stdin();
                let mut stdout = io::stdout();
                for line in stdin.lock().lines() {
                    let line = line.map_err(|source| CliError::Io {
                        context: "read MCP stdin".to_owned(),
                        source,
                    })?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    if let Some(response) = server.handle_line(&line) {
                        writeln!(stdout, "{response}").map_err(|source| CliError::Io {
                            context: "write MCP stdout".to_owned(),
                            source,
                        })?;
                        stdout.flush().map_err(|source| CliError::Io {
                            context: "flush MCP stdout".to_owned(),
                            source,
                        })?;
                    }
                }
            }
        },
    }
    Ok(0)
}

fn workspace(
    service: &LocalizeService<SqliteStore>,
    command: &WorkspaceCommand,
    json: bool,
) -> Result<(), CliError> {
    match command {
        WorkspaceCommand::Create {
            name,
            slug,
            description,
        } => {
            let workspace = service.create_workspace(CreateWorkspaceInput {
                name: name.clone(),
                slug: slug.clone(),
                description: description.clone(),
            })?;
            print_value(json, &workspace, || {
                println!(
                    "Created workspace {} (slug: {}, id: {})",
                    workspace.name, workspace.slug, workspace.id
                );
            })?;
        }
        WorkspaceCommand::List => {
            let workspaces = service.list_workspaces()?;
            print_value(json, &workspaces, || {
                if workspaces.is_empty() {
                    println!("No workspaces yet. Run: localize workspace create demo");
                } else {
                    print_table(
                        &["SLUG", "NAME", "ID"],
                        &workspaces
                            .iter()
                            .map(|workspace| {
                                vec![
                                    workspace.slug.clone(),
                                    workspace.name.clone(),
                                    workspace.id.to_string(),
                                ]
                            })
                            .collect::<Vec<_>>(),
                    );
                }
            })?;
        }
        WorkspaceCommand::Show { workspace } => {
            let workspace = resolve_workspace(service, workspace.as_deref())?;
            print_value(json, &workspace, || {
                println!("{} ({})", workspace.name, workspace.slug);
                println!("id: {}", workspace.id);
                if let Some(description) = &workspace.description {
                    println!("description: {description}");
                }
                println!("created: {}", workspace.created_at.to_rfc3339());
            })?;
        }
    }
    Ok(())
}

fn project(
    service: &LocalizeService<SqliteStore>,
    command: &ProjectCommand,
    json: bool,
) -> Result<(), CliError> {
    match command {
        ProjectCommand::Create {
            name,
            workspace,
            slug,
            description,
            default_locale,
        } => {
            let workspace = resolve_workspace(service, workspace.as_deref())?;
            let project = service.create_project(
                workspace.id,
                CreateProjectInput {
                    name: name.clone(),
                    slug: slug.clone(),
                    description: description.clone(),
                    default_locale: default_locale.clone(),
                },
            )?;
            print_value(json, &project, || {
                println!(
                    "Created project {} (slug: {}, workspace: {}, id: {})",
                    project.name, project.slug, workspace.slug, project.id
                );
                println!("Default environments created: development, staging, production");
                if let Some(locale) = &project.default_locale {
                    println!("Default locale: {locale}");
                }
            })?;
        }
        ProjectCommand::List { workspace } => {
            let workspace = resolve_workspace(service, workspace.as_deref())?;
            let projects = service.list_projects(workspace.id)?;
            print_value(json, &projects, || {
                if projects.is_empty() {
                    println!(
                        "No projects in {}. Run: localize project create demo-app",
                        workspace.slug
                    );
                } else {
                    print_table(
                        &["SLUG", "NAME", "DEFAULT LOCALE", "ID"],
                        &projects
                            .iter()
                            .map(|project| {
                                vec![
                                    project.slug.clone(),
                                    project.name.clone(),
                                    project
                                        .default_locale
                                        .clone()
                                        .unwrap_or_else(|| "-".to_owned()),
                                    project.id.to_string(),
                                ]
                            })
                            .collect::<Vec<_>>(),
                    );
                }
            })?;
        }
    }
    Ok(())
}

fn locale(
    service: &LocalizeService<SqliteStore>,
    command: &LocaleCommand,
    json: bool,
) -> Result<(), CliError> {
    match command {
        LocaleCommand::Add {
            code,
            workspace,
            project,
            name,
            default,
        } => {
            let project = resolve_project(service, workspace.as_deref(), project.as_deref())?;
            let locale = service.add_locale(
                project.id,
                AddLocaleInput {
                    code: code.clone(),
                    name: name.clone(),
                    is_default: *default,
                },
            )?;
            print_value(json, &locale, || {
                println!(
                    "Added locale {} ({}). Default: {}",
                    locale.code,
                    locale.name,
                    if locale.is_default { "yes" } else { "no" }
                );
            })?;
        }
        LocaleCommand::List { workspace, project } => {
            let project = resolve_project(service, workspace.as_deref(), project.as_deref())?;
            let locales = service.list_locales(project.id)?;
            print_value(json, &locales, || {
                if locales.is_empty() {
                    println!("No locales. Run: localize locale add en-US");
                } else {
                    print_table(
                        &["CODE", "NAME", "DEFAULT", "ID"],
                        &locales
                            .iter()
                            .map(|locale| {
                                vec![
                                    locale.code.clone(),
                                    locale.name.clone(),
                                    if locale.is_default { "yes" } else { "no" }.to_owned(),
                                    locale.id.to_string(),
                                ]
                            })
                            .collect::<Vec<_>>(),
                    );
                }
            })?;
        }
        LocaleCommand::SetDefault {
            code,
            workspace,
            project,
        } => {
            let project = resolve_project(service, workspace.as_deref(), project.as_deref())?;
            let locale = service.set_default_locale(project.id, code)?;
            print_value(json, &locale, || {
                println!("Default locale is now {}", locale.code);
            })?;
        }
    }
    Ok(())
}

fn namespace(
    service: &LocalizeService<SqliteStore>,
    command: &NamespaceCommand,
    json: bool,
) -> Result<(), CliError> {
    match command {
        NamespaceCommand::Create {
            name,
            workspace,
            project,
        } => {
            let project = resolve_project(service, workspace.as_deref(), project.as_deref())?;
            let namespace = service
                .create_namespace(project.id, CreateNamespaceInput { name: name.clone() })?;
            print_value(json, &namespace, || {
                println!("Created namespace {} ({})", namespace.name, namespace.id);
            })?;
        }
        NamespaceCommand::List { workspace, project } => {
            let project = resolve_project(service, workspace.as_deref(), project.as_deref())?;
            let namespaces = service.list_namespaces(project.id)?;
            print_value(json, &namespaces, || {
                print_table(
                    &["NAME", "ID"],
                    &namespaces
                        .iter()
                        .map(|namespace| vec![namespace.name.clone(), namespace.id.to_string()])
                        .collect::<Vec<_>>(),
                );
            })?;
        }
    }
    Ok(())
}

fn keys(
    service: &LocalizeService<SqliteStore>,
    command: &KeysCommand,
    json: bool,
) -> Result<(), CliError> {
    let KeysCommand::List {
        workspace,
        project,
        locale,
        namespace,
    } = command;
    let project = resolve_project(service, workspace.as_deref(), project.as_deref())?;
    if let Some(locale) = locale {
        let entries = service.list_translations(project.id, locale, namespace.as_deref())?;
        print_value(json, &entries, || {
            if entries.is_empty() {
                println!("No translation keys. Add one or import fixtures/locales.");
            } else {
                print_table(
                    &["NAMESPACE", "KEY", "VALUE", "STATUS"],
                    &entries
                        .iter()
                        .map(|entry| {
                            vec![
                                entry.namespace.clone(),
                                entry.key.clone(),
                                entry
                                    .value
                                    .clone()
                                    .unwrap_or_else(|| "(missing)".to_owned()),
                                entry
                                    .status
                                    .map(|status| status.as_str().to_owned())
                                    .unwrap_or_else(|| "-".to_owned()),
                            ]
                        })
                        .collect::<Vec<_>>(),
                );
            }
        })?;
    } else {
        let keys = service.list_keys(project.id)?;
        print_value(json, &keys, || {
            if keys.is_empty() {
                println!("No translation keys. Add one or run: localize import fixtures/locales");
            } else {
                print_table(
                    &["NAMESPACE", "KEY", "DESCRIPTION"],
                    &keys
                        .iter()
                        .map(|key| {
                            vec![
                                key.namespace.clone(),
                                key.key.clone(),
                                key.description.clone().unwrap_or_else(|| "-".to_owned()),
                            ]
                        })
                        .collect::<Vec<_>>(),
                );
            }
        })?;
    }
    Ok(())
}

fn translation(
    service: &LocalizeService<SqliteStore>,
    command: &TranslationCommand,
    json: bool,
) -> Result<(), CliError> {
    match command {
        TranslationCommand::Set {
            key,
            value,
            locale,
            namespace,
            workspace,
            project,
            status,
            description,
        } => {
            let project = resolve_project(service, workspace.as_deref(), project.as_deref())?;
            let entry = service.save_translation(SaveTranslationInput {
                project_id: project.id,
                namespace: namespace.clone(),
                key: key.clone(),
                locale_code: locale.clone(),
                value: value.clone(),
                status: TranslationStatus::parse(status)?,
                description: description.clone(),
            })?;
            print_value(json, &entry, || {
                println!(
                    "Saved {}/{} [{}] = {:?}",
                    entry.namespace, entry.key, entry.locale_code, entry.value
                );
            })?;
        }
        TranslationCommand::Get {
            key,
            locale,
            namespace,
            workspace,
            project,
        } => {
            let project = resolve_project(service, workspace.as_deref(), project.as_deref())?;
            let entry = service.get_translation(project.id, locale, namespace, key)?;
            match entry {
                Some(entry) => print_value(json, &entry, || {
                    println!(
                        "{} [{}] {} = {:?}",
                        entry.key,
                        entry.locale_code,
                        entry
                            .status
                            .map(|status| status.as_str())
                            .unwrap_or("missing"),
                        entry.value
                    );
                })?,
                None => {
                    return Err(DomainError::not_found(
                        "translation",
                        format!("{namespace}/{key} [{locale}]"),
                    )
                    .into());
                }
            }
        }
    }
    Ok(())
}

fn import(
    service: &LocalizeService<SqliteStore>,
    command: &ImportCommand,
    json: bool,
) -> Result<(), CliError> {
    let project = resolve_project(
        service,
        command.workspace.as_deref(),
        command.project.as_deref(),
    )?;
    let report = service.import_translations(
        project.id,
        ImportOptions {
            path: command.path.clone(),
            namespace: command.namespace.clone(),
        },
    )?;
    print_value(json, &report, || {
        println!(
            "Imported {} files: {} locales created, {} keys created, {} values written",
            report.files_processed,
            report.locales_created,
            report.keys_created,
            report.values_written
        );
        if !report.skipped_files.is_empty() {
            println!("Skipped: {}", report.skipped_files.join(", "));
        }
    })?;
    Ok(())
}

fn export(
    service: &LocalizeService<SqliteStore>,
    command: &ExportCommand,
    json: bool,
) -> Result<(), CliError> {
    if command.format != "json" {
        return Err(DomainError::Unsupported {
            feature: format!(
                "export format {} (only json is implemented)",
                command.format
            ),
        }
        .into());
    }
    let project = resolve_project(
        service,
        command.workspace.as_deref(),
        command.project.as_deref(),
    )?;
    let output = command
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from("localize-export"));
    let report = service.export_translations(
        project.id,
        ExportOptions {
            output_dir: output,
            format: ExportFormat::Json,
            nested: command.nested,
            namespace: command.namespace.clone(),
        },
    )?;
    print_value(json, &report, || {
        println!(
            "Exported {} files ({} values) to {}",
            report.files.len(),
            report.total_keys,
            report.output_dir.display()
        );
        for file in &report.files {
            println!(
                "  {} ({} keys)",
                report.output_dir.join(&file.relative_path).display(),
                file.key_count
            );
        }
    })?;
    Ok(())
}

fn check(
    service: &LocalizeService<SqliteStore>,
    command: &CheckCommand,
    json: bool,
) -> Result<u8, CliError> {
    let project = resolve_project(
        service,
        command.workspace.as_deref(),
        command.project.as_deref(),
    )?;
    let report = service.check_project(project.id)?;
    let has_issues = report.has_issues();
    print_value(json, &report, || {
        println!(
            "Check {}: {} keys, {} locales, {} missing translations, {} placeholder mismatches",
            project.slug,
            report.total_keys,
            report.locales.len(),
            report.missing.len(),
            report.placeholder_mismatches.len()
        );
        if report.default_locale.is_none() {
            println!("warning: no default locale is configured; missing-key validation is skipped");
        }
        for missing in &report.missing {
            println!(
                "  missing  {}  {}/{}",
                missing.locale_code, missing.namespace, missing.key
            );
        }
        for mismatch in &report.placeholder_mismatches {
            println!(
                "  placeholder {}  {}/{} expected {:?}, got {:?}",
                mismatch.locale_code,
                mismatch.namespace,
                mismatch.key,
                mismatch.expected,
                mismatch.actual
            );
        }
        if !has_issues {
            println!("OK");
        }
    })?;
    Ok(if has_issues { 1 } else { 0 })
}

fn environment(
    service: &LocalizeService<SqliteStore>,
    command: &EnvCommand,
    json: bool,
) -> Result<(), CliError> {
    match command {
        EnvCommand::List { workspace, project } => {
            let project = resolve_project(service, workspace.as_deref(), project.as_deref())?;
            let environments = service.list_environments(project.id)?;
            print_value(json, &environments, || {
                print_table(
                    &["NAME", "KIND", "ID"],
                    &environments
                        .iter()
                        .map(|environment| {
                            vec![
                                environment.name.clone(),
                                environment.kind.as_str().to_owned(),
                                environment.id.to_string(),
                            ]
                        })
                        .collect::<Vec<_>>(),
                );
            })?;
        }
        EnvCommand::Create {
            name,
            kind,
            workspace,
            project,
        } => {
            let project = resolve_project(service, workspace.as_deref(), project.as_deref())?;
            let environment = service.create_environment(
                project.id,
                CreateEnvironmentInput {
                    name: name.clone(),
                    kind: EnvironmentKind::parse(kind)?,
                },
            )?;
            print_value(json, &environment, || {
                println!(
                    "Created environment {} ({})",
                    environment.name,
                    environment.kind.as_str()
                );
            })?;
        }
    }
    Ok(())
}

fn config(
    service: &LocalizeService<SqliteStore>,
    command: &ConfigCommand,
    json: bool,
) -> Result<(), CliError> {
    match command {
        ConfigCommand::Set {
            key,
            value,
            environment,
            visibility,
            workspace,
            project,
        } => {
            let project = resolve_project(service, workspace.as_deref(), project.as_deref())?;
            let environment = resolve_environment(service, &project, environment)?;
            let variable = service.set_config_variable(SetConfigVariableInput {
                environment_id: environment.id,
                key: key.clone(),
                value: value.clone(),
                visibility: ConfigVisibility::parse(visibility)?,
            })?;
            print_value(json, &variable, || {
                println!(
                    "Set {} [{}] in {} ({})",
                    variable.key,
                    variable.visibility.as_str(),
                    environment.name,
                    variable.value
                );
            })?;
        }
        ConfigCommand::List {
            environment,
            workspace,
            project,
        } => {
            let project = resolve_project(service, workspace.as_deref(), project.as_deref())?;
            let environment = resolve_environment(service, &project, environment)?;
            let variables = service.list_config_variables(environment.id)?;
            print_value(json, &variables, || {
                if variables.is_empty() {
                    println!("No variables in {}.", environment.name);
                } else {
                    print_table(
                        &["KEY", "VALUE", "VISIBILITY"],
                        &variables
                            .iter()
                            .map(|variable| {
                                vec![
                                    variable.key.clone(),
                                    variable.value.clone(),
                                    variable.visibility.as_str().to_owned(),
                                ]
                            })
                            .collect::<Vec<_>>(),
                    );
                }
            })?;
        }
    }
    Ok(())
}

fn database(
    service: &LocalizeService<SqliteStore>,
    command: &DbCommand,
    db_path: &std::path::Path,
    json: bool,
) -> Result<(), CliError> {
    match command {
        DbCommand::Migrate => {
            let version = service.store().schema_version()?;
            print_value(json, &json!({ "schemaVersion": version }), || {
                println!(
                    "Database is at schema version {version}: {}",
                    db_path.display()
                );
            })?;
        }
        DbCommand::Status => {
            let stats = service.store().stats()?;
            print_value(json, &stats, || print_stats(&stats))?;
        }
        DbCommand::Backup { out } => {
            let target = out.clone().unwrap_or_else(|| {
                let parent = db_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."));
                parent.join("backups").join(format!(
                    "slimlocalize-{}.sqlite3",
                    Local::now().format("%Y%m%d-%H%M%S")
                ))
            });
            let path = service.store().backup_to(target)?;
            print_value(json, &json!({ "backup": path }), || {
                println!("Backup written to {}", path.display());
            })?;
        }
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorReport {
    name: &'static str,
    version: &'static str,
    os: &'static str,
    arch: &'static str,
    database_path: String,
    database_exists: bool,
    schema_version: i64,
    sqlite_version: String,
    journal_mode: String,
    stats: StorageStats,
    checks: Vec<DoctorCheck>,
    blockers: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorCheck {
    name: &'static str,
    status: &'static str,
    detail: String,
}

fn doctor(
    service: &LocalizeService<SqliteStore>,
    db_path: &std::path::Path,
    json: bool,
) -> Result<(), CliError> {
    let store = service.store();
    let stats = store.stats()?;
    let schema_version = store.schema_version()?;
    let journal_mode = store.journal_mode()?;
    let database_exists = db_path.exists();
    let writable = db_path
        .parent()
        .is_none_or(|parent| parent.exists() && parent.is_dir());
    let mut checks = vec![
        DoctorCheck {
            name: "database",
            status: if database_exists { "ok" } else { "warn" },
            detail: if database_exists {
                db_path.display().to_string()
            } else {
                format!("will be created at {}", db_path.display())
            },
        },
        DoctorCheck {
            name: "schema",
            status: if schema_version >= 1 { "ok" } else { "error" },
            detail: format!("version {schema_version}"),
        },
        DoctorCheck {
            name: "journal",
            status: if journal_mode.eq_ignore_ascii_case("wal") {
                "ok"
            } else {
                "warn"
            },
            detail: journal_mode.clone(),
        },
        DoctorCheck {
            name: "data-directory",
            status: if writable { "ok" } else { "error" },
            detail: db_path
                .parent()
                .map(|parent| parent.display().to_string())
                .unwrap_or_else(|| ".".to_owned()),
        },
        DoctorCheck {
            name: "foreign-keys",
            status: "ok",
            detail: "enforced by the SQLite adapter".to_owned(),
        },
    ];
    if let Err(error) = store.health_check() {
        checks.push(DoctorCheck {
            name: "quick-check",
            status: "error",
            detail: error.to_string(),
        });
    } else {
        checks.push(DoctorCheck {
            name: "quick-check",
            status: "ok",
            detail: "PRAGMA quick_check returned ok".to_owned(),
        });
    }
    let blockers = checks
        .iter()
        .filter(|check| check.status == "error")
        .count();
    let report = DoctorReport {
        name: "SlimLocalize",
        version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        database_path: db_path.display().to_string(),
        database_exists,
        schema_version,
        sqlite_version: store.sqlite_version(),
        journal_mode,
        stats,
        checks,
        blockers,
    };
    if json {
        print_json(&report)?;
    } else {
        println!(
            "SlimLocalize {} ({} {})",
            report.version, report.os, report.arch
        );
        println!("database: {}", report.database_path);
        println!(
            "sqlite: {} | schema {} | journal {}",
            report.sqlite_version, report.schema_version, report.journal_mode
        );
        println!(
            "rows: {} workspaces, {} projects, {} keys, {} values",
            report.stats.workspaces,
            report.stats.projects,
            report.stats.translation_keys,
            report.stats.translation_values
        );
        for check in &report.checks {
            println!("  [{}] {}: {}", check.status, check.name, check.detail);
        }
        println!(
            "blockers: {} (this command inspects the runtime database; use `just doctor` for system tooling)",
            report.blockers
        );
    }
    if blockers > 0 {
        return Err(
            DomainError::internal(format!("{blockers} blocking doctor check(s) failed")).into(),
        );
    }
    Ok(())
}

fn resolve_workspace(
    service: &LocalizeService<SqliteStore>,
    reference: Option<&str>,
) -> Result<Workspace, DomainError> {
    if let Some(reference) = reference {
        return service.resolve_workspace(reference);
    }
    let workspaces = service.list_workspaces()?;
    match workspaces.len() {
        1 => Ok(workspaces.into_iter().next().expect("one workspace")),
        0 => Err(DomainError::not_found(
            "workspace",
            "none exist; run `localize workspace create demo`",
        )),
        _ => Err(DomainError::validation(
            "workspace",
            "multiple workspaces exist; pass --workspace <slug> or --workspace <id>",
        )),
    }
}

fn resolve_project(
    service: &LocalizeService<SqliteStore>,
    workspace: Option<&str>,
    project: Option<&str>,
) -> Result<Project, DomainError> {
    let workspace = resolve_workspace(service, workspace)?;
    if let Some(project) = project {
        return service.resolve_project(workspace.id, project);
    }
    let projects = service.list_projects(workspace.id)?;
    match projects.len() {
        1 => Ok(projects.into_iter().next().expect("one project")),
        0 => Err(DomainError::not_found(
            "project",
            "none exist; run `localize project create demo-app`",
        )),
        _ => Err(DomainError::validation(
            "project",
            "multiple projects exist; pass --project <slug> or --project <id>",
        )),
    }
}

fn resolve_environment(
    service: &LocalizeService<SqliteStore>,
    project: &Project,
    name: &str,
) -> Result<Environment, DomainError> {
    service
        .list_environments(project.id)?
        .into_iter()
        .find(|environment| environment.name == name)
        .ok_or_else(|| {
            DomainError::not_found("environment", format!("{name} (project {})", project.slug))
        })
}

fn init_tracing(level: &str) {
    let filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(io::stderr)
        .with_target(false)
        .compact()
        .try_init();
}

fn print_value<T: Serialize>(json: bool, value: &T, human: impl FnOnce()) -> Result<(), CliError> {
    if json {
        print_json(value)?;
    } else {
        human();
    }
    Ok(())
}

fn print_json<T: Serialize>(value: &T) -> Result<(), CliError> {
    let serialized = serde_json::to_string_pretty(value)?;
    println!("{serialized}");
    Ok(())
}

fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    let mut widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();
    for row in rows {
        for (index, value) in row.iter().enumerate() {
            if let Some(width) = widths.get_mut(index) {
                *width = (*width).max(value.chars().count());
            }
        }
    }
    let render = |row: &[String]| {
        row.iter()
            .enumerate()
            .map(|(index, value)| format!("{value:<width$}", width = widths[index]))
            .collect::<Vec<_>>()
            .join("  ")
    };
    println!(
        "{}",
        render(&headers.iter().map(ToString::to_string).collect::<Vec<_>>())
    );
    println!(
        "{}",
        widths
            .iter()
            .map(|width| "-".repeat(*width))
            .collect::<Vec<_>>()
            .join("  ")
    );
    for row in rows {
        println!("{}", render(row));
    }
}

fn print_stats(stats: &StorageStats) {
    println!("database: {}", stats.database_path);
    println!(
        "sqlite {} | schema {} | journal {} | {} bytes",
        stats.sqlite_version, stats.schema_version, stats.journal_mode, stats.file_size_bytes
    );
    println!(
        "rows: workspaces={} projects={} locales={} keys={} values={} environments={} config={} audit={}",
        stats.workspaces,
        stats.projects,
        stats.locales,
        stats.translation_keys,
        stats.translation_values,
        stats.environments,
        stats.config_variables,
        stats.audit_events
    );
}

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("{0}")]
    Domain(#[from] DomainError),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
}

impl CliError {
    fn code(&self) -> &'static str {
        match self {
            Self::Domain(error) => error.code(),
            Self::Json(_) => "invalid_json",
            Self::Io { .. } => "io_error",
        }
    }
}
