use std::{fs, path::PathBuf, process::Command};

use serde_json::Value;
use uuid::Uuid;

fn spcli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_spcli"))
}

fn isolated_config_dir() -> PathBuf {
    std::env::temp_dir().join(format!("spcli-test-{}", Uuid::new_v4().simple()))
}

fn run_spcli(args: &[&str]) -> (i32, String, String) {
    let config_dir = isolated_config_dir();
    let output = spcli()
        .args(args)
        .env("XDG_CONFIG_HOME", &config_dir)
        .env("HOME", &config_dir)
        .output()
        .expect("spcli command should run");
    let _ = fs::remove_dir_all(&config_dir);
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

#[test]
fn manifest_json_is_machine_readable() {
    let (code, stdout, stderr) = run_spcli(&["--json", "manifest"]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stderr.trim().is_empty());
    let manifest: Value = serde_json::from_str(&stdout).expect("manifest must be JSON");
    assert_eq!(manifest["name"], "spcli");
    assert_eq!(manifest["schema_version"], "1");

    let commands = manifest["commands"]
        .as_array()
        .expect("commands must be an array");
    assert!(commands.iter().any(|command| {
        command["name"].as_str() == Some("finance transactions get")
            && command["auth_required"].as_bool() == Some(true)
            && command["company_required"].as_bool() == Some(true)
            && command["destructive"].as_bool() == Some(false)
    }));
    assert!(commands.iter().any(|command| {
        command["name"].as_str() == Some("cfdi get")
            && command["auth_required"].as_bool() == Some(true)
            && command["company_required"].as_bool() == Some(true)
            && command["destructive"].as_bool() == Some(false)
    }));
    assert!(commands.iter().any(|command| {
        command["name"].as_str() == Some("cfdi jobs status")
            && command["auth_required"].as_bool() == Some(true)
            && command["company_required"].as_bool() == Some(true)
            && command["destructive"].as_bool() == Some(false)
    }));
    assert!(commands.iter().any(|command| {
        command["name"].as_str() == Some("resources usages allocations replace")
            && command["auth_required"].as_bool() == Some(true)
            && command["company_required"].as_bool() == Some(true)
            && command["destructive"].as_bool() == Some(false)
    }));
    assert!(commands.iter().any(|command| {
        command["name"].as_str() == Some("projects concepts advance")
            && command["auth_required"].as_bool() == Some(true)
            && command["company_required"].as_bool() == Some(true)
            && command["destructive"].as_bool() == Some(false)
    }));
    assert!(commands.iter().any(|command| {
        command["name"].as_str() == Some("finance transactions create")
            && command["auth_required"].as_bool() == Some(true)
            && command["company_required"].as_bool() == Some(true)
            && command["destructive"].as_bool() == Some(false)
    }));
    assert!(commands.iter().any(|command| {
        command["name"].as_str() == Some("finance planned-entries pay")
            && command["auth_required"].as_bool() == Some(true)
            && command["company_required"].as_bool() == Some(true)
            && command["destructive"].as_bool() == Some(false)
    }));
}

#[test]
fn invalid_object_id_returns_structured_validation_error() {
    let (code, stdout, stderr) = run_spcli(&["--json", "finance", "accounts", "get", "not-an-id"]);

    assert_eq!(code, 2);
    assert!(stdout.trim().is_empty());
    let error: Value = serde_json::from_str(&stderr).expect("stderr must be JSON");
    assert_eq!(error["code"], "validation_error");
    assert!(
        error["message"]
            .as_str()
            .unwrap_or_default()
            .contains("ObjectId")
    );
}

#[test]
fn destructive_delete_requires_confirmation_before_auth() {
    let (code, stdout, stderr) = run_spcli(&[
        "--json",
        "finance",
        "accounts",
        "delete",
        "64f000000000000000000000",
    ]);

    assert_eq!(code, 2);
    assert!(stdout.trim().is_empty());
    let error: Value = serde_json::from_str(&stderr).expect("stderr must be JSON");
    assert_eq!(error["code"], "confirmation_required");
    assert!(
        error["message"]
            .as_str()
            .unwrap_or_default()
            .contains("--yes")
    );
}

#[test]
fn missing_credentials_return_not_authenticated() {
    let (code, stdout, stderr) = run_spcli(&["--json", "status"]);

    assert_eq!(code, 3);
    assert!(stdout.trim().is_empty());
    let error: Value = serde_json::from_str(&stderr).expect("stderr must be JSON");
    assert_eq!(error["code"], "not_authenticated");
    assert!(
        error["message"]
            .as_str()
            .unwrap_or_default()
            .contains("spcli login")
    );
}
