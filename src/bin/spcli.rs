use std::{env, fs, path::PathBuf, process::ExitCode};

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use alfredodev::totp::build_totp;
use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};
use rand::RngCore;
use reqwest::{Client, StatusCode, header};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const APP_NAME: &str = "spcli";
const CREDENTIAL_FILE: &str = "credentials.bin";
const ENVELOPE_MAGIC: &[u8; 8] = b"SPCLI01\0";
const NONCE_LEN: usize = 12;
const SALT_LEN: usize = 32;

#[derive(Parser)]
#[command(name = "spcli")]
#[command(about = "CLI client for alfredodev APIs")]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Login(LoginArgs),
    Status,
    Logout,
    ResetAuth(ResetAuthArgs),
    Company {
        #[command(subcommand)]
        command: CompanyCommand,
    },
    Manifest,
}

#[derive(Args)]
struct LoginArgs {
    #[arg(long)]
    base_url: String,
    #[arg(long)]
    email: String,
    #[arg(long)]
    totp_secret: String,
}

#[derive(Args)]
struct ResetAuthArgs {
    #[arg(long)]
    yes: bool,
}

#[derive(Subcommand)]
enum CompanyCommand {
    List,
    Use { slug: String },
}

#[derive(Debug, Serialize)]
struct CliError {
    code: &'static str,
    message: String,
    details: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CredentialState {
    base_url: String,
    email: String,
    totp_secret: String,
    session_cookie: Option<String>,
    company_slug: Option<String>,
    tenant_host: Option<String>,
    last_login_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LoginResponse {
    ok: bool,
    redirect_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CompanySummary {
    id: String,
    name: String,
    slug: String,
    active: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let cli_error = CliError {
                code: classify_error(&err),
                message: err.to_string(),
                details: err.chain().skip(1).map(|cause| cause.to_string()).collect(),
            };
            let rendered = serde_json::to_string_pretty(&cli_error)
                .unwrap_or_else(|_| format!("{}", cli_error.message));
            eprintln!("{rendered}");
            ExitCode::from(1)
        }
    }
}

async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Login(args) => login(args, cli.json).await,
        Command::Status => status(cli.json).await,
        Command::Logout => logout(cli.json).await,
        Command::ResetAuth(args) => reset_auth(args, cli.json),
        Command::Company { command } => match command {
            CompanyCommand::List => company_list(cli.json).await,
            CompanyCommand::Use { slug } => company_use(&slug, cli.json).await,
        },
        Command::Manifest => print_manifest(cli.json),
    }
}

async fn login(args: LoginArgs, json_output: bool) -> Result<()> {
    let base_url = normalize_base_url(&args.base_url)?;
    let mut state = CredentialState {
        base_url,
        email: args.email,
        totp_secret: args.totp_secret,
        session_cookie: None,
        company_slug: None,
        tenant_host: None,
        last_login_at: None,
    };
    refresh_login(&mut state).await?;
    save_state(&state)?;

    if json_output {
        print_json(&json!({
            "ok": true,
            "email": state.email,
            "base_url": state.base_url,
            "credential_path": credential_path()?.display().to_string()
        }))?;
    } else {
        println!("Logged in as {}", state.email);
        println!("Credential file: {}", credential_path()?.display());
    }
    Ok(())
}

async fn status(json_output: bool) -> Result<()> {
    let mut state = load_state()?;
    let value = authenticated_get(&mut state, "/setup").await?;
    save_state(&state)?;
    if json_output {
        print_json(&value)?;
    } else {
        println!(
            "Authenticated: {}",
            value["email"].as_str().unwrap_or("unknown")
        );
        println!(
            "Company: {}",
            value["company"].as_str().unwrap_or("unknown")
        );
        println!("Role: {}", value["role"].as_str().unwrap_or("unknown"));
    }
    Ok(())
}

async fn logout(json_output: bool) -> Result<()> {
    let mut state = load_state()?;
    if state.session_cookie.is_some() {
        let _ = post_logout(&state).await;
    }
    state.session_cookie = None;
    state.last_login_at = None;
    save_state(&state)?;
    if json_output {
        print_json(&json!({ "ok": true, "credential_removed": false }))?;
    } else {
        println!("Session cleared. Stored TOTP secret remains configured.");
    }
    Ok(())
}

fn reset_auth(args: ResetAuthArgs, json_output: bool) -> Result<()> {
    if !args.yes {
        bail!("reset-auth removes stored credentials; rerun with --yes to confirm");
    }
    let path = credential_path()?;
    match fs::remove_file(&path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err).context("failed to remove credential file"),
    }
    if json_output {
        print_json(&json!({ "ok": true, "credential_removed": true }))?;
    } else {
        println!("Credentials removed.");
    }
    Ok(())
}

async fn company_list(json_output: bool) -> Result<()> {
    let mut state = load_state()?;
    let companies: Vec<CompanySummary> =
        serde_json::from_value(authenticated_get(&mut state, "/api/me/companies").await?)
            .context("failed to parse company list")?;
    save_state(&state)?;
    if json_output {
        print_json(&companies)?;
    } else if companies.is_empty() {
        println!("No companies available.");
    } else {
        for company in companies {
            let marker = if Some(company.slug.as_str()) == state.company_slug.as_deref() {
                "*"
            } else {
                " "
            };
            println!("{marker} {} ({})", company.slug, company.name);
        }
    }
    Ok(())
}

async fn company_use(slug: &str, json_output: bool) -> Result<()> {
    let mut state = load_state()?;
    let companies: Vec<CompanySummary> =
        serde_json::from_value(authenticated_get(&mut state, "/api/me/companies").await?)
            .context("failed to parse company list")?;
    let selected = companies
        .iter()
        .find(|company| company.slug.eq_ignore_ascii_case(slug))
        .ok_or_else(|| anyhow!("company '{slug}' is not available to this user"))?;
    state.company_slug = Some(selected.slug.clone());
    state.tenant_host = Some(derive_tenant_host(&state.base_url, &selected.slug)?);
    save_state(&state)?;

    if json_output {
        print_json(&json!({
            "ok": true,
            "company": selected,
            "tenant_host": state.tenant_host
        }))?;
    } else {
        println!("Using company {} ({})", selected.slug, selected.name);
    }
    Ok(())
}

fn print_manifest(json_output: bool) -> Result<()> {
    let manifest = json!({
        "name": "spcli",
        "commands": [
            { "name": "login", "auth_required": false, "company_required": false, "destructive": false },
            { "name": "status", "auth_required": true, "company_required": false, "destructive": false },
            { "name": "logout", "auth_required": false, "company_required": false, "destructive": false },
            { "name": "reset-auth", "auth_required": false, "company_required": false, "destructive": true, "confirmation_flag": "--yes" },
            { "name": "company list", "auth_required": true, "company_required": false, "destructive": false },
            { "name": "company use", "auth_required": true, "company_required": false, "destructive": false },
            { "name": "manifest", "auth_required": false, "company_required": false, "destructive": false }
        ],
        "output": { "human_default": true, "json_flag": "--json" }
    });
    if json_output {
        print_json(&manifest)?;
    } else {
        println!(
            "spcli commands: login, status, logout, reset-auth, company list, company use, manifest"
        );
        println!("Use --json for machine-readable output.");
    }
    Ok(())
}

async fn authenticated_get(state: &mut CredentialState, path: &str) -> Result<Value> {
    ensure_session(state).await?;
    let response = get_with_session(state, path).await?;
    if response.status() == StatusCode::UNAUTHORIZED {
        refresh_login(state).await?;
        let retry = get_with_session(state, path).await?;
        return parse_json_response(retry).await;
    }
    parse_json_response(response).await
}

async fn ensure_session(state: &mut CredentialState) -> Result<()> {
    if state.session_cookie.is_none() {
        refresh_login(state).await?;
    }
    Ok(())
}

async fn get_with_session(state: &CredentialState, path: &str) -> Result<reqwest::Response> {
    let client = Client::new();
    let url = endpoint(&request_base_url(state)?, path)?;
    let mut request = client
        .get(url)
        .header(header::COOKIE, session_cookie_header(state)?);
    if let Some(host) = request_host(state) {
        request = request.header(header::HOST, host);
    }
    request.send().await.context("request failed")
}

async fn refresh_login(state: &mut CredentialState) -> Result<()> {
    let code = current_totp_code(state)?;
    let client = Client::new();
    let response = client
        .post(endpoint(&state.base_url, "/login")?)
        .header(header::HOST, base_host(&state.base_url)?)
        .json(&json!({ "email": state.email, "code": code }))
        .send()
        .await
        .context("login request failed")?;
    if response.status() == StatusCode::UNAUTHORIZED {
        bail!("login failed");
    }
    if !response.status().is_success() {
        bail!("login failed with status {}", response.status());
    }
    let session = extract_session_cookie(response.headers())?;
    let body: LoginResponse = response
        .json()
        .await
        .context("failed to parse login response")?;
    if !body.ok {
        bail!("login failed");
    }
    state.session_cookie = Some(session);
    state.last_login_at = Some(chrono::Utc::now().to_rfc3339());
    Ok(())
}

async fn post_logout(state: &CredentialState) -> Result<()> {
    let client = Client::new();
    let mut request = client
        .post(endpoint(&request_base_url(state)?, "/logout")?)
        .header(header::COOKIE, session_cookie_header(state)?);
    if let Some(host) = request_host(state) {
        request = request.header(header::HOST, host);
    }
    let response = request.send().await.context("logout request failed")?;
    if !response.status().is_success() && response.status() != StatusCode::UNAUTHORIZED {
        bail!("logout failed with status {}", response.status());
    }
    Ok(())
}

async fn parse_json_response(response: reqwest::Response) -> Result<Value> {
    let status = response.status();
    let body = response
        .text()
        .await
        .context("failed to read response body")?;
    if !status.is_success() {
        bail!("request failed with status {status}: {body}");
    }
    serde_json::from_str(&body).with_context(|| format!("failed to parse JSON response: {body}"))
}

fn current_totp_code(state: &CredentialState) -> Result<String> {
    let totp = build_totp(APP_NAME, &state.email, &state.totp_secret)
        .context("failed to build TOTP from stored secret")?;
    totp.generate_current()
        .map_err(|err| anyhow!("failed to generate TOTP code: {err}"))
}

fn save_state(state: &CredentialState) -> Result<()> {
    let path = credential_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("failed to create credential directory")?;
        set_private_dir_permissions(parent)?;
    }
    let plaintext = serde_json::to_vec(state).context("failed to serialize credentials")?;
    let encrypted = encrypt_envelope(&plaintext)?;
    fs::write(&path, encrypted).context("failed to write credential file")?;
    set_private_file_permissions(&path)?;
    Ok(())
}

fn load_state() -> Result<CredentialState> {
    let path = credential_path()?;
    let data = fs::read(&path).with_context(|| {
        format!(
            "credential file not found at {}; run spcli login",
            path.display()
        )
    })?;
    decrypt_envelope(&data)
}

#[allow(deprecated)]
fn encrypt_envelope(plaintext: &[u8]) -> Result<Vec<u8>> {
    let mut salt = [0_u8; SALT_LEN];
    let mut nonce = [0_u8; NONCE_LEN];
    rand::rng().fill_bytes(&mut salt);
    rand::rng().fill_bytes(&mut nonce);
    let key = derive_key(&salt);
    let cipher = Aes256Gcm::new_from_slice(&key).context("failed to initialize cipher")?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|_| anyhow!("failed to encrypt credential envelope"))?;
    let mut output =
        Vec::with_capacity(ENVELOPE_MAGIC.len() + SALT_LEN + NONCE_LEN + ciphertext.len());
    output.extend_from_slice(ENVELOPE_MAGIC);
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

#[allow(deprecated)]
fn decrypt_envelope(data: &[u8]) -> Result<CredentialState> {
    let min_len = ENVELOPE_MAGIC.len() + SALT_LEN + NONCE_LEN;
    if data.len() <= min_len || &data[..ENVELOPE_MAGIC.len()] != ENVELOPE_MAGIC {
        bail!("credential envelope is invalid or unsupported");
    }
    let salt_start = ENVELOPE_MAGIC.len();
    let nonce_start = salt_start + SALT_LEN;
    let ciphertext_start = nonce_start + NONCE_LEN;
    let salt = &data[salt_start..nonce_start];
    let nonce = &data[nonce_start..ciphertext_start];
    let ciphertext = &data[ciphertext_start..];

    let key = derive_key(salt);
    let cipher = Aes256Gcm::new_from_slice(&key).context("failed to initialize cipher")?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| anyhow!("credential envelope cannot be decrypted; run spcli login again"))?;
    serde_json::from_slice(&plaintext).context("failed to parse credential envelope")
}

fn derive_key(salt: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"spcli credential envelope v1");
    hasher.update(salt);
    hasher.update(local_machine_material().as_bytes());
    hasher.finalize().into()
}

fn local_machine_material() -> String {
    let machine_id = fs::read_to_string("/etc/machine-id")
        .or_else(|_| fs::read_to_string("/var/lib/dbus/machine-id"))
        .unwrap_or_default();
    let user = env::var("USER")
        .or_else(|_| env::var("USERNAME"))
        .unwrap_or_default();
    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .unwrap_or_default();
    format!("{machine_id}\n{user}\n{home}")
}

fn credential_path() -> Result<PathBuf> {
    let base =
        dirs::config_dir().ok_or_else(|| anyhow!("could not locate user config directory"))?;
    Ok(base.join(APP_NAME).join(CREDENTIAL_FILE))
}

fn normalize_base_url(base_url: &str) -> Result<String> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        bail!("base URL cannot be empty");
    }
    let url = reqwest::Url::parse(trimmed).context("base URL is invalid")?;
    match url.scheme() {
        "http" | "https" => Ok(trimmed.to_string()),
        scheme => bail!("unsupported URL scheme '{scheme}'"),
    }
}

fn endpoint(base_url: &str, path: &str) -> Result<String> {
    Ok(format!("{}{}", normalize_base_url(base_url)?, path))
}

fn base_host(base_url: &str) -> Result<String> {
    let url = reqwest::Url::parse(base_url).context("base URL is invalid")?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("base URL has no host"))?;
    Ok(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    })
}

fn derive_tenant_host(base_url: &str, slug: &str) -> Result<String> {
    let url = reqwest::Url::parse(base_url).context("base URL is invalid")?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("base URL has no host"))?;
    let root_host = host.strip_prefix("app.").unwrap_or(host);
    let tenant_host = format!("{slug}.{root_host}");
    Ok(match url.port() {
        Some(port) => format!("{tenant_host}:{port}"),
        None => tenant_host,
    })
}

fn request_base_url(state: &CredentialState) -> Result<String> {
    let mut url = reqwest::Url::parse(&state.base_url).context("base URL is invalid")?;
    let host = request_host(state).ok_or_else(|| anyhow!("request host is unavailable"))?;
    let (host_no_port, port) = host
        .split_once(':')
        .map(|(h, p)| (h, p.parse::<u16>().ok()))
        .unwrap_or((host.as_str(), None));
    url.set_host(Some(host_no_port))
        .map_err(|_| anyhow!("failed to set request host"))?;
    url.set_port(port)
        .map_err(|_| anyhow!("failed to set request port"))?;
    Ok(url.as_str().trim_end_matches('/').to_string())
}

fn request_host(state: &CredentialState) -> Option<String> {
    state
        .tenant_host
        .clone()
        .or_else(|| base_host(&state.base_url).ok())
}

fn session_cookie_header(state: &CredentialState) -> Result<String> {
    let cookie = state
        .session_cookie
        .as_deref()
        .ok_or_else(|| anyhow!("missing session cookie"))?;
    Ok(format!("session={cookie}"))
}

fn extract_session_cookie(headers: &header::HeaderMap) -> Result<String> {
    for value in headers.get_all(header::SET_COOKIE) {
        let cookie = value.to_str().context("invalid Set-Cookie header")?;
        if let Some(rest) = cookie.strip_prefix("session=") {
            let token = rest.split(';').next().unwrap_or_default().trim();
            if !token.is_empty() {
                return Ok(token.to_string());
            }
        }
    }
    bail!("login response did not include a session cookie")
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn classify_error(err: &anyhow::Error) -> &'static str {
    let message = err.to_string();
    if message.contains("credential") {
        "credential_error"
    } else if message.contains("login") || message.contains("unauthorized") {
        "auth_error"
    } else if message.contains("company") {
        "company_error"
    } else {
        "spcli_error"
    }
}

#[cfg(unix)]
fn set_private_file_permissions(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &std::path::Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_dir_permissions(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_dir_permissions(_path: &std::path::Path) -> Result<()> {
    Ok(())
}
