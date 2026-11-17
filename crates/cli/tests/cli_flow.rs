use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

struct CliFixture {
    _dir: TempDir,
    db: std::path::PathBuf,
}

impl CliFixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("cli.sqlite3");
        Self { _dir: dir, db }
    }

    fn command(&self) -> Command {
        let mut command = Command::cargo_bin("localize").unwrap();
        command.arg("--db").arg(&self.db);
        command
    }

    fn json(&self, args: &[&str]) -> (i32, Value) {
        let output = self
            .command()
            .arg("--json")
            .args(args)
            .output()
            .expect("run CLI");
        let value: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
            panic!(
                "invalid JSON.\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        });
        (output.status.code().unwrap_or(-1), value)
    }
}

#[test]
fn cli_full_flow_uses_real_sqlite_and_persists() {
    let fixture = CliFixture::new();
    let (code, workspace) = fixture.json(&["workspace", "create", "Demo"]);
    assert_eq!(code, 0);
    assert_eq!(workspace["slug"], "demo");

    let (code, project) = fixture.json(&["project", "create", "Demo App", "--workspace", "demo"]);
    assert_eq!(code, 0);
    assert_eq!(project["slug"], "demo-app");

    let (code, locale) = fixture.json(&[
        "locale",
        "add",
        "en-US",
        "--workspace",
        "demo",
        "--project",
        "demo-app",
    ]);
    assert_eq!(code, 0);
    assert_eq!(locale["isDefault"], true);

    let (code, _) = fixture.json(&[
        "locale",
        "add",
        "es-PE",
        "--workspace",
        "demo",
        "--project",
        "demo-app",
    ]);
    assert_eq!(code, 0);

    let (code, _) = fixture.json(&[
        "translation",
        "set",
        "welcome",
        "Welcome, {name}!",
        "--locale",
        "en-US",
        "--workspace",
        "demo",
        "--project",
        "demo-app",
        "--status",
        "translated",
    ]);
    assert_eq!(code, 0);

    let (code, _) = fixture.json(&[
        "translation",
        "set",
        "welcome",
        "¡Bienvenido!",
        "--locale",
        "es-PE",
        "--workspace",
        "demo",
        "--project",
        "demo-app",
    ]);
    assert_eq!(code, 0);

    let (code, keys) = fixture.json(&[
        "keys",
        "list",
        "--locale",
        "es-PE",
        "--workspace",
        "demo",
        "--project",
        "demo-app",
    ]);
    assert_eq!(code, 0);
    assert_eq!(keys.as_array().unwrap().len(), 1);
    assert_eq!(keys[0]["value"], "¡Bienvenido!");

    let (code, report) = fixture.json(&["check", "--workspace", "demo", "--project", "demo-app"]);
    assert_eq!(code, 1, "check exits 1 when placeholder mismatches exist");
    assert_eq!(report["placeholderMismatches"].as_array().unwrap().len(), 1);

    let export_dir = fixture._dir.path().join("out");
    let (code, report) = fixture.json(&[
        "export",
        "--workspace",
        "demo",
        "--project",
        "demo-app",
        "--out",
        export_dir.to_str().unwrap(),
        "--nested",
    ]);
    assert_eq!(code, 0);
    assert_eq!(report["files"].as_array().unwrap().len(), 2);
    let exported: Value =
        serde_json::from_str(&std::fs::read_to_string(export_dir.join("en-US.json")).unwrap())
            .unwrap();
    assert_eq!(exported["welcome"], "Welcome, {name}!");

    // Each invocation is a new process: this proves on-disk persistence.
    let (code, workspaces) = fixture.json(&["workspace", "list"]);
    assert_eq!(code, 0);
    assert_eq!(workspaces.as_array().unwrap().len(), 1);

    let (code, doctor) = fixture.json(&["doctor"]);
    assert_eq!(code, 0);
    assert_eq!(doctor["schemaVersion"], 1);
    assert_eq!(doctor["stats"]["translationKeys"], 1);
}

#[test]
fn cli_import_and_env_config_flow() {
    let fixture = CliFixture::new();
    fixture.json(&["workspace", "create", "Import"]);
    fixture.json(&[
        "project",
        "create",
        "Fixture",
        "--workspace",
        "import",
        "--default-locale",
        "en-US",
    ]);
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/locales")
        .canonicalize()
        .unwrap();
    let (code, report) = fixture.json(&[
        "import",
        fixtures.to_str().unwrap(),
        "--workspace",
        "import",
        "--project",
        "fixture",
    ]);
    assert_eq!(code, 0);
    assert_eq!(report["filesProcessed"], 3);
    assert_eq!(report["keysCreated"], 5);

    let (code, _) = fixture.json(&[
        "config",
        "set",
        "PUBLIC_API_URL",
        "http://localhost:3000",
        "--environment",
        "development",
        "--workspace",
        "import",
        "--project",
        "fixture",
    ]);
    assert_eq!(code, 0);
    let (code, variables) = fixture.json(&[
        "config",
        "list",
        "--environment",
        "development",
        "--workspace",
        "import",
        "--project",
        "fixture",
    ]);
    assert_eq!(code, 0);
    assert_eq!(variables[0]["visibility"], "public");

    let (code, envs) = fixture.json(&[
        "env",
        "list",
        "--workspace",
        "import",
        "--project",
        "fixture",
    ]);
    assert_eq!(code, 0);
    assert_eq!(envs.as_array().unwrap().len(), 3);

    let (code, report) = fixture.json(&["check", "--workspace", "import", "--project", "fixture"]);
    assert_eq!(code, 1);
    assert!(!report["missing"].as_array().unwrap().is_empty());
}

#[test]
fn cli_rejects_secret_visibility_and_bad_locale() {
    let fixture = CliFixture::new();
    fixture.json(&["workspace", "create", "Security"]);
    fixture.json(&["project", "create", "App", "--workspace", "security"]);
    fixture.json(&[
        "locale",
        "add",
        "en-US",
        "--workspace",
        "security",
        "--project",
        "app",
    ]);
    let (code, error) = fixture.json(&[
        "config",
        "set",
        "TOKEN",
        "secret-value",
        "--environment",
        "development",
        "--visibility",
        "secret",
        "--workspace",
        "security",
        "--project",
        "app",
    ]);
    assert_eq!(code, 1);
    assert_eq!(error["error"]["code"], "unsupported");

    let (code, error) = fixture.json(&[
        "locale",
        "add",
        "../../etc",
        "--workspace",
        "security",
        "--project",
        "app",
    ]);
    assert_eq!(code, 1);
    assert_eq!(error["error"]["code"], "validation_error");
}

#[test]
fn cli_help_and_version_work_without_a_database() {
    Command::cargo_bin("localize")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("localize"))
        .stdout(predicate::str::contains("doctor"));
    Command::cargo_bin("localize")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("0.1.0"));
}

#[test]
fn cli_mcp_lists_read_only_tools() {
    let fixture = CliFixture::new();
    let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
    let output = fixture
        .command()
        .arg("mcp")
        .arg("serve")
        .write_stdin(format!("{request}\n"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    let tools = response["result"]["tools"].as_array().unwrap();
    assert!(tools.len() >= 8);
    assert!(tools.iter().all(|tool| tool["readOnly"] == true));
    assert!(
        !tools
            .iter()
            .any(|tool| tool["name"] == "propose_translation")
    );
}
