#![recursion_limit = "256"]

use std::{env, fs, io::Read, path::PathBuf, process::ExitCode};

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
    Finance {
        #[command(subcommand)]
        command: FinanceCommand,
    },
    Cfdi {
        #[command(subcommand)]
        command: CfdiCommand,
    },
    Sat {
        #[command(subcommand)]
        command: SatCommand,
    },
    Projects {
        #[command(subcommand)]
        command: ProjectsCommand,
    },
    Resources {
        #[command(subcommand)]
        command: ResourcesCommand,
    },
    Time {
        #[command(subcommand)]
        command: TimeCommand,
    },
    Pdf {
        #[command(subcommand)]
        command: PdfCommand,
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

#[derive(Subcommand)]
enum FinanceCommand {
    Accounts {
        #[command(subcommand)]
        command: AccountCommand,
    },
    Categories {
        #[command(subcommand)]
        command: CategoryCommand,
    },
    Contacts {
        #[command(subcommand)]
        command: ContactCommand,
    },
    Forecasts {
        #[command(subcommand)]
        command: ForecastCommand,
    },
    RecurringPlans {
        #[command(subcommand)]
        command: RecurringPlanCommand,
    },
    PlannedEntries {
        #[command(subcommand)]
        command: PlannedEntryCommand,
    },
    Transactions {
        #[command(subcommand)]
        command: TransactionCommand,
    },
}

#[derive(Subcommand)]
enum TransactionCommand {
    List,
    Get { id: String },
    Create(TransactionWriteArgs),
    Update(TransactionUpdateArgs),
    Delete(DeleteArgs),
}

#[derive(Subcommand)]
enum PlannedEntryCommand {
    List,
    Get { id: String },
    Create(PlannedEntryWriteArgs),
    Update(PlannedEntryUpdateArgs),
    Delete(DeleteArgs),
    Pay(PlannedEntryPayArgs),
    BulkPay(PlannedEntryBulkPayArgs),
}

#[derive(Subcommand)]
enum RecurringPlanCommand {
    List,
    Get { id: String },
    Create(RecurringPlanWriteArgs),
    Update(RecurringPlanUpdateArgs),
    Delete(DeleteArgs),
    Generate { id: String },
}

#[derive(Args)]
struct RecurringPlanWriteArgs {
    #[arg(long)]
    name: String,
    #[arg(long)]
    flow_type: String,
    #[arg(long)]
    category_id: String,
    #[arg(long)]
    account_expected_id: String,
    #[arg(long)]
    contact_id: Option<String>,
    #[arg(long)]
    amount_estimated: f64,
    #[arg(long, default_value = "monthly")]
    frequency: String,
    #[arg(long)]
    day_of_month: Option<i32>,
    #[arg(long)]
    start_date: String,
    #[arg(long)]
    end_date: Option<String>,
    #[arg(long)]
    inactive: bool,
    #[arg(long, default_value_t = 1)]
    version: i32,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Args)]
struct RecurringPlanUpdateArgs {
    id: String,
    #[command(flatten)]
    fields: RecurringPlanWriteArgs,
}

#[derive(Args)]
struct PlannedEntryWriteArgs {
    #[arg(long)]
    name: String,
    #[arg(long)]
    flow_type: String,
    #[arg(long)]
    category_id: String,
    #[arg(long)]
    account_expected_id: String,
    #[arg(long)]
    contact_id: Option<String>,
    #[arg(long)]
    project_id: Option<String>,
    #[arg(long)]
    amount_estimated: f64,
    #[arg(long)]
    due_date: String,
    #[arg(long, default_value = "planned")]
    status: String,
    #[arg(long)]
    recurring_plan_id: Option<String>,
    #[arg(long)]
    recurring_plan_version: Option<i32>,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Args)]
struct PlannedEntryUpdateArgs {
    id: String,
    #[command(flatten)]
    fields: PlannedEntryWriteArgs,
}

#[derive(Args)]
struct PlannedEntryPayArgs {
    id: String,
    #[arg(long)]
    paid_at: String,
    #[arg(long)]
    amount: f64,
    #[arg(long)]
    account_id: String,
    #[arg(long)]
    project_id: Option<String>,
    #[arg(long)]
    parent_planned_entry_id: Option<String>,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Args)]
struct PlannedEntryBulkPayArgs {
    #[arg(long = "entry-id", required = true)]
    entry_ids: Vec<String>,
    #[arg(long)]
    paid_at: String,
    #[arg(long)]
    account_id: String,
    #[arg(long)]
    project_id: Option<String>,
    #[arg(long)]
    parent_planned_entry_id: Option<String>,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Args)]
struct TransactionWriteArgs {
    #[arg(long)]
    date: String,
    #[arg(long)]
    description: String,
    #[arg(long)]
    transaction_type: String,
    #[arg(long)]
    category_id: String,
    #[arg(long)]
    account_from_id: Option<String>,
    #[arg(long)]
    account_to_id: Option<String>,
    #[arg(long)]
    amount: f64,
    #[arg(long)]
    planned_entry_id: Option<String>,
    #[arg(long)]
    unconfirmed: bool,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Args)]
struct TransactionUpdateArgs {
    id: String,
    #[command(flatten)]
    fields: TransactionWriteArgs,
}

#[derive(Subcommand)]
enum CfdiCommand {
    List,
    Get {
        uuid: String,
    },
    Jobs {
        #[command(subcommand)]
        command: CfdiJobsCommand,
    },
}

#[derive(Subcommand)]
enum CfdiJobsCommand {
    List,
    Status { job_id: String },
}

#[derive(Subcommand)]
enum SatCommand {
    Configs {
        #[command(subcommand)]
        command: SatConfigsCommand,
    },
}

#[derive(Subcommand)]
enum SatConfigsCommand {
    List,
    Get { id: String },
}

#[derive(Subcommand)]
enum ProjectsCommand {
    List,
    Get {
        id: String,
    },
    StatusSummary(ProjectStatusSummaryArgs),
    Statuses {
        #[command(subcommand)]
        command: ProjectStatusesCommand,
    },
    Concepts {
        #[command(subcommand)]
        command: ProjectConceptsCommand,
    },
}

#[derive(Args)]
struct ProjectStatusSummaryArgs {
    #[arg(long)]
    project_id: String,
}

#[derive(Subcommand)]
enum ProjectStatusesCommand {
    List,
    Create(ProjectStatusWriteArgs),
    Update(ProjectStatusUpdateArgs),
    Delete(DeleteArgs),
}

#[derive(Args)]
struct ProjectStatusWriteArgs {
    #[arg(long)]
    name: String,
    #[arg(long)]
    position: i32,
    #[arg(long)]
    color: Option<String>,
    #[arg(long)]
    initial: bool,
    #[arg(long)]
    terminal: bool,
    #[arg(long)]
    cancelled: bool,
    #[arg(long)]
    inactive: bool,
}

#[derive(Args)]
struct ProjectStatusUpdateArgs {
    id: String,
    #[command(flatten)]
    fields: ProjectStatusWriteArgs,
}

#[derive(Subcommand)]
enum ProjectConceptsCommand {
    List(ProjectConceptsListArgs),
    Create(ProjectConceptCreateArgs),
    Update(ProjectConceptUpdateArgs),
    Delete(DeleteArgs),
    Advance { id: String },
}

#[derive(Args)]
struct ProjectConceptsListArgs {
    #[arg(long)]
    project_id: String,
}

#[derive(Args)]
struct ProjectConceptWriteArgs {
    #[arg(long)]
    name: String,
    #[arg(long)]
    quantity: f64,
    #[arg(long)]
    status_id: Option<String>,
    #[arg(long)]
    unit: Option<String>,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    estimated_hours: Option<f64>,
    #[arg(long)]
    estimated_cost: Option<f64>,
    #[arg(long)]
    notes: Option<String>,
    #[arg(long, default_value_t = 0)]
    position: i32,
}

#[derive(Args)]
struct ProjectConceptCreateArgs {
    #[arg(long)]
    project_id: String,
    #[command(flatten)]
    fields: ProjectConceptWriteArgs,
}

#[derive(Args)]
struct ProjectConceptUpdateArgs {
    id: String,
    #[command(flatten)]
    fields: ProjectConceptWriteArgs,
}

#[derive(Subcommand)]
enum ResourcesCommand {
    List,
    Get {
        id: String,
    },
    Logs {
        #[command(subcommand)]
        command: ResourceLogsCommand,
    },
    Usages {
        #[command(subcommand)]
        command: ResourceUsagesCommand,
    },
}

#[derive(Subcommand)]
enum ResourceLogsCommand {
    List,
    Get { id: String },
}

#[derive(Subcommand)]
enum ResourceUsagesCommand {
    List,
    Get {
        id: String,
    },
    Create(ResourceUsageCreateArgs),
    Update(ResourceUsageUpdateArgs),
    Delete(DeleteArgs),
    Allocations {
        #[command(subcommand)]
        command: ResourceUsageAllocationsCommand,
    },
}

#[derive(Subcommand)]
enum ResourceUsageAllocationsCommand {
    List { usage_id: String },
    Replace(ResourceUsageAllocationsReplaceArgs),
}

#[derive(Args)]
struct ResourceUsageCreateArgs {
    #[arg(long)]
    resource_id: String,
    #[arg(long)]
    started_at: String,
    #[arg(long)]
    ended_at: Option<String>,
    #[arg(long)]
    operator_name: Option<String>,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Args)]
struct ResourceUsageUpdateArgs {
    id: String,
    #[arg(long)]
    started_at: String,
    #[arg(long)]
    ended_at: Option<String>,
    #[arg(long)]
    hourly_cost_snapshot: f64,
    #[arg(long)]
    operator_name: Option<String>,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Args)]
struct ResourceUsageAllocationsReplaceArgs {
    usage_id: String,
    #[arg(long = "concept-id")]
    concept_ids: Vec<String>,
    #[arg(long = "allocation")]
    allocations: Vec<String>,
}

#[derive(Subcommand)]
enum TimeCommand {
    Timeline(TimelineArgs),
}

#[derive(Args)]
struct TimelineArgs {
    #[arg(long)]
    mode: String,
    #[arg(long)]
    from: String,
    #[arg(long)]
    to: String,
}

#[derive(Subcommand)]
enum PdfCommand {
    Preview(PdfPreviewArgs),
}

#[derive(Args)]
struct PdfPreviewArgs {
    #[arg(long, conflicts_with = "source")]
    input: Option<PathBuf>,
    #[arg(long)]
    source: Option<String>,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Subcommand)]
enum ListCommand {
    List,
}

#[derive(Subcommand)]
enum ListGetCommand {
    List,
    Get { id: String },
}

#[derive(Subcommand)]
enum AccountCommand {
    List,
    Get { id: String },
    Create(AccountCreateArgs),
    Update(AccountUpdateArgs),
    Delete(DeleteArgs),
}

#[derive(Args)]
struct AccountCreateArgs {
    #[arg(long)]
    name: String,
    #[arg(long)]
    account_type: String,
    #[arg(long)]
    currency: Option<String>,
    #[arg(long)]
    inactive: bool,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Args)]
struct AccountUpdateArgs {
    id: String,
    #[arg(long)]
    name: String,
    #[arg(long)]
    account_type: String,
    #[arg(long)]
    currency: Option<String>,
    #[arg(long)]
    inactive: bool,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Subcommand)]
enum CategoryCommand {
    List,
    Get { id: String },
    Create(CategoryCreateArgs),
    Update(CategoryUpdateArgs),
    Delete(DeleteArgs),
}

#[derive(Args)]
struct CategoryCreateArgs {
    #[arg(long)]
    name: String,
    #[arg(long)]
    flow_type: String,
    #[arg(long)]
    parent_id: Option<String>,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Args)]
struct CategoryUpdateArgs {
    id: String,
    #[arg(long)]
    name: String,
    #[arg(long)]
    flow_type: String,
    #[arg(long)]
    parent_id: Option<String>,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Subcommand)]
enum ContactCommand {
    List,
    Get { id: String },
    Create(ContactCreateArgs),
    Update(ContactUpdateArgs),
    Delete(DeleteArgs),
}

#[derive(Args)]
struct ContactCreateArgs {
    #[arg(long)]
    name: String,
    #[arg(long)]
    contact_type: String,
    #[arg(long)]
    rfc: Option<String>,
    #[arg(long)]
    email: Option<String>,
    #[arg(long)]
    phone: Option<String>,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Args)]
struct ContactUpdateArgs {
    id: String,
    #[arg(long)]
    name: String,
    #[arg(long)]
    contact_type: String,
    #[arg(long)]
    rfc: Option<String>,
    #[arg(long)]
    email: Option<String>,
    #[arg(long)]
    phone: Option<String>,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Args)]
struct DeleteArgs {
    id: String,
    #[arg(long)]
    yes: bool,
}

#[derive(Subcommand)]
enum ForecastCommand {
    List,
    Get { id: String },
    Create(ForecastWriteArgs),
    Update(ForecastUpdateArgs),
    Delete(DeleteArgs),
}

#[derive(Args)]
struct ForecastWriteArgs {
    #[arg(long)]
    generated_at: String,
    #[arg(long)]
    start_date: String,
    #[arg(long)]
    end_date: String,
    #[arg(long)]
    currency: String,
    #[arg(long)]
    projected_income_total: f64,
    #[arg(long)]
    projected_expense_total: f64,
    #[arg(long)]
    projected_net: f64,
    #[arg(long)]
    initial_balance: Option<f64>,
    #[arg(long)]
    final_balance: Option<f64>,
    #[arg(long)]
    generated_by_user_id: Option<String>,
    #[arg(long)]
    details: Option<String>,
    #[arg(long)]
    scenario_name: Option<String>,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Args)]
struct ForecastUpdateArgs {
    id: String,
    #[command(flatten)]
    fields: ForecastWriteArgs,
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
            ExitCode::from(exit_code(cli_error.code))
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
        Command::Finance { command } => match command {
            FinanceCommand::Accounts { command } => match command {
                AccountCommand::List => {
                    json_get_command("/api/admin/accounts", cli.json, "accounts").await
                }
                AccountCommand::Get { id } => {
                    json_get_by_id_command("/api/admin/accounts", &id, cli.json, "account").await
                }
                AccountCommand::Create(args) => account_create(args, cli.json).await,
                AccountCommand::Update(args) => account_update(args, cli.json).await,
                AccountCommand::Delete(args) => {
                    delete_command("/api/admin/accounts", args, cli.json, "account").await
                }
            },
            FinanceCommand::Categories { command } => match command {
                CategoryCommand::List => {
                    json_get_command("/api/admin/categories", cli.json, "categories").await
                }
                CategoryCommand::Get { id } => {
                    json_get_by_id_command("/api/admin/categories", &id, cli.json, "category").await
                }
                CategoryCommand::Create(args) => category_create(args, cli.json).await,
                CategoryCommand::Update(args) => category_update(args, cli.json).await,
                CategoryCommand::Delete(args) => {
                    delete_command("/api/admin/categories", args, cli.json, "category").await
                }
            },
            FinanceCommand::Contacts { command } => match command {
                ContactCommand::List => {
                    json_get_command("/api/admin/contacts", cli.json, "contacts").await
                }
                ContactCommand::Get { id } => {
                    json_get_by_id_command("/api/admin/contacts", &id, cli.json, "contact").await
                }
                ContactCommand::Create(args) => contact_create(args, cli.json).await,
                ContactCommand::Update(args) => contact_update(args, cli.json).await,
                ContactCommand::Delete(args) => {
                    delete_command("/api/admin/contacts", args, cli.json, "contact").await
                }
            },
            FinanceCommand::Forecasts { command } => match command {
                ForecastCommand::List => {
                    json_get_command("/api/admin/forecasts", cli.json, "forecasts").await
                }
                ForecastCommand::Get { id } => {
                    json_get_by_id_command("/api/admin/forecasts", &id, cli.json, "forecast").await
                }
                ForecastCommand::Create(args) => forecast_create(args, cli.json).await,
                ForecastCommand::Update(args) => forecast_update(args, cli.json).await,
                ForecastCommand::Delete(args) => {
                    delete_command("/api/admin/forecasts", args, cli.json, "forecast").await
                }
            },
            FinanceCommand::RecurringPlans { command } => match command {
                RecurringPlanCommand::List => {
                    json_get_command("/api/admin/recurring-plans", cli.json, "recurring plans")
                        .await
                }
                RecurringPlanCommand::Get { id } => {
                    json_get_by_id_command(
                        "/api/admin/recurring-plans",
                        &id,
                        cli.json,
                        "recurring plan",
                    )
                    .await
                }
                RecurringPlanCommand::Create(args) => recurring_plan_create(args, cli.json).await,
                RecurringPlanCommand::Update(args) => recurring_plan_update(args, cli.json).await,
                RecurringPlanCommand::Delete(args) => {
                    delete_command(
                        "/api/admin/recurring-plans",
                        args,
                        cli.json,
                        "recurring plan",
                    )
                    .await
                }
                RecurringPlanCommand::Generate { id } => {
                    recurring_plan_generate(id, cli.json).await
                }
            },
            FinanceCommand::PlannedEntries { command } => match command {
                PlannedEntryCommand::List => {
                    json_get_command("/api/admin/planned-entries", cli.json, "planned entries")
                        .await
                }
                PlannedEntryCommand::Get { id } => {
                    json_get_by_id_command(
                        "/api/admin/planned-entries",
                        &id,
                        cli.json,
                        "planned entry",
                    )
                    .await
                }
                PlannedEntryCommand::Create(args) => planned_entry_create(args, cli.json).await,
                PlannedEntryCommand::Update(args) => planned_entry_update(args, cli.json).await,
                PlannedEntryCommand::Delete(args) => {
                    delete_command(
                        "/api/admin/planned-entries",
                        args,
                        cli.json,
                        "planned entry",
                    )
                    .await
                }
                PlannedEntryCommand::Pay(args) => planned_entry_pay(args, cli.json).await,
                PlannedEntryCommand::BulkPay(args) => {
                    planned_entries_bulk_pay(args, cli.json).await
                }
            },
            FinanceCommand::Transactions { command } => match command {
                TransactionCommand::List => {
                    json_get_command("/api/admin/transactions/data", cli.json, "transactions").await
                }
                TransactionCommand::Get { id } => {
                    json_get_by_id_command("/api/admin/transactions", &id, cli.json, "transaction")
                        .await
                }
                TransactionCommand::Create(args) => transaction_create(args, cli.json).await,
                TransactionCommand::Update(args) => transaction_update(args, cli.json).await,
                TransactionCommand::Delete(args) => {
                    delete_command("/api/admin/transactions", args, cli.json, "transaction").await
                }
            },
        },
        Command::Cfdi { command } => match command {
            CfdiCommand::List => json_get_command("/api/admin/cfdis/data", cli.json, "CFDIs").await,
            CfdiCommand::Get { uuid } => cfdi_get(&uuid, cli.json).await,
            CfdiCommand::Jobs { command } => match command {
                CfdiJobsCommand::List => cfdi_jobs_list(cli.json).await,
                CfdiJobsCommand::Status { job_id } => cfdi_job_status(&job_id, cli.json).await,
            },
        },
        Command::Sat { command } => match command {
            SatCommand::Configs { command } => match command {
                SatConfigsCommand::List => {
                    json_get_command("/api/admin/sat-configs", cli.json, "SAT configs").await
                }
                SatConfigsCommand::Get { id } => {
                    json_get_by_id_command("/api/admin/sat-configs", &id, cli.json, "SAT config")
                        .await
                }
            },
        },
        Command::Projects { command } => match command {
            ProjectsCommand::List => {
                json_get_command("/api/admin/projects", cli.json, "projects").await
            }
            ProjectsCommand::Get { id } => {
                json_get_by_id_command("/api/admin/projects", &id, cli.json, "project").await
            }
            ProjectsCommand::StatusSummary(args) => project_status_summary(args, cli.json).await,
            ProjectsCommand::Statuses { command } => match command {
                ProjectStatusesCommand::List => {
                    json_get_command("/api/admin/concept_statuses", cli.json, "concept statuses")
                        .await
                }
                ProjectStatusesCommand::Create(args) => project_status_create(args, cli.json).await,
                ProjectStatusesCommand::Update(args) => project_status_update(args, cli.json).await,
                ProjectStatusesCommand::Delete(args) => {
                    delete_command(
                        "/api/admin/concept_statuses",
                        args,
                        cli.json,
                        "concept status",
                    )
                    .await
                }
            },
            ProjectsCommand::Concepts { command } => match command {
                ProjectConceptsCommand::List(args) => project_concepts_list(args, cli.json).await,
                ProjectConceptsCommand::Create(args) => {
                    project_concept_create(args, cli.json).await
                }
                ProjectConceptsCommand::Update(args) => {
                    project_concept_update(args, cli.json).await
                }
                ProjectConceptsCommand::Delete(args) => {
                    delete_command(
                        "/api/admin/project_concepts",
                        args,
                        cli.json,
                        "project concept",
                    )
                    .await
                }
                ProjectConceptsCommand::Advance { id } => {
                    project_concept_advance(&id, cli.json).await
                }
            },
        },
        Command::Resources { command } => match command {
            ResourcesCommand::List => {
                json_get_command("/api/admin/resources", cli.json, "resources").await
            }
            ResourcesCommand::Get { id } => {
                json_get_by_id_command("/api/admin/resources", &id, cli.json, "resource").await
            }
            ResourcesCommand::Logs { command } => match command {
                ResourceLogsCommand::List => {
                    json_get_command("/api/admin/resource_logs", cli.json, "resource logs").await
                }
                ResourceLogsCommand::Get { id } => {
                    json_get_by_id_command(
                        "/api/admin/resource_logs",
                        &id,
                        cli.json,
                        "resource log",
                    )
                    .await
                }
            },
            ResourcesCommand::Usages { command } => match command {
                ResourceUsagesCommand::List => {
                    json_get_command("/api/admin/resource_usages", cli.json, "resource usages")
                        .await
                }
                ResourceUsagesCommand::Get { id } => {
                    json_get_by_id_command(
                        "/api/admin/resource_usages",
                        &id,
                        cli.json,
                        "resource usage",
                    )
                    .await
                }
                ResourceUsagesCommand::Create(args) => resource_usage_create(args, cli.json).await,
                ResourceUsagesCommand::Update(args) => resource_usage_update(args, cli.json).await,
                ResourceUsagesCommand::Delete(args) => {
                    delete_command(
                        "/api/admin/resource_usages",
                        args,
                        cli.json,
                        "resource usage",
                    )
                    .await
                }
                ResourceUsagesCommand::Allocations { command } => match command {
                    ResourceUsageAllocationsCommand::List { usage_id } => {
                        resource_usage_allocations_list(&usage_id, cli.json).await
                    }
                    ResourceUsageAllocationsCommand::Replace(args) => {
                        resource_usage_allocations_replace(args, cli.json).await
                    }
                },
            },
        },
        Command::Time { command } => match command {
            TimeCommand::Timeline(args) => timeline(args, cli.json).await,
        },
        Command::Pdf { command } => match command {
            PdfCommand::Preview(args) => pdf_preview(args, cli.json).await,
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
        print_json(&sanitized_status(value))?;
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

fn sanitized_status(mut value: Value) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.remove("otpauth_url");
    }
    value
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

async fn json_get_command(path: &str, json_output: bool, label: &str) -> Result<()> {
    let mut state = load_state()?;
    let value = authenticated_get(&mut state, path).await?;
    save_state(&state)?;
    print_value_output(&value, json_output, label)
}

async fn json_get_by_id_command(
    base_path: &str,
    id: &str,
    json_output: bool,
    label: &str,
) -> Result<()> {
    validate_object_id(id, "id")?;
    let path = format!("{base_path}/{id}");
    json_get_command(&path, json_output, label).await
}

async fn account_create(args: AccountCreateArgs, json_output: bool) -> Result<()> {
    validate_non_empty(&args.name, "name")?;
    validate_non_empty(&args.account_type, "account-type")?;
    let mut state = load_state()?;
    let value = authenticated_post_json(
        &mut state,
        "/api/admin/accounts",
        &json!({
            "name": args.name,
            "account_type": args.account_type,
            "currency": args.currency,
            "is_active": !args.inactive,
            "notes": args.notes,
        }),
    )
    .await?;
    save_state(&state)?;
    print_created_output(&value, json_output, "account")
}

async fn account_update(args: AccountUpdateArgs, json_output: bool) -> Result<()> {
    validate_object_id(&args.id, "id")?;
    validate_non_empty(&args.name, "name")?;
    validate_non_empty(&args.account_type, "account-type")?;
    let path = format!("/api/admin/accounts/{}/update", args.id);
    let mut state = load_state()?;
    let value = authenticated_post_json(
        &mut state,
        &path,
        &json!({
            "name": args.name,
            "account_type": args.account_type,
            "currency": args.currency,
            "is_active": !args.inactive,
            "notes": args.notes,
        }),
    )
    .await?;
    save_state(&state)?;
    print_ok_output(&value, json_output, "account updated")
}

async fn category_create(args: CategoryCreateArgs, json_output: bool) -> Result<()> {
    validate_non_empty(&args.name, "name")?;
    validate_non_empty(&args.flow_type, "flow-type")?;
    if let Some(parent_id) = args.parent_id.as_deref() {
        validate_object_id(parent_id, "parent-id")?;
    }
    let mut state = load_state()?;
    let value = authenticated_post_json(
        &mut state,
        "/api/admin/categories",
        &json!({
            "name": args.name,
            "flow_type": args.flow_type,
            "parent_id": args.parent_id,
            "notes": args.notes,
        }),
    )
    .await?;
    save_state(&state)?;
    print_created_output(&value, json_output, "category")
}

async fn category_update(args: CategoryUpdateArgs, json_output: bool) -> Result<()> {
    validate_object_id(&args.id, "id")?;
    validate_non_empty(&args.name, "name")?;
    validate_non_empty(&args.flow_type, "flow-type")?;
    if let Some(parent_id) = args.parent_id.as_deref() {
        validate_object_id(parent_id, "parent-id")?;
    }
    let path = format!("/api/admin/categories/{}/update", args.id);
    let mut state = load_state()?;
    let value = authenticated_post_json(
        &mut state,
        &path,
        &json!({
            "name": args.name,
            "flow_type": args.flow_type,
            "parent_id": args.parent_id,
            "notes": args.notes,
        }),
    )
    .await?;
    save_state(&state)?;
    print_ok_output(&value, json_output, "category updated")
}

async fn contact_create(args: ContactCreateArgs, json_output: bool) -> Result<()> {
    validate_non_empty(&args.name, "name")?;
    validate_non_empty(&args.contact_type, "contact-type")?;
    let mut state = load_state()?;
    let value = authenticated_post_json(
        &mut state,
        "/api/admin/contacts",
        &json!({
            "name": args.name,
            "contact_type": args.contact_type,
            "rfc": args.rfc,
            "email": args.email,
            "phone": args.phone,
            "notes": args.notes,
        }),
    )
    .await?;
    save_state(&state)?;
    print_created_output(&value, json_output, "contact")
}

async fn contact_update(args: ContactUpdateArgs, json_output: bool) -> Result<()> {
    validate_object_id(&args.id, "id")?;
    validate_non_empty(&args.name, "name")?;
    validate_non_empty(&args.contact_type, "contact-type")?;
    let path = format!("/api/admin/contacts/{}/update", args.id);
    let mut state = load_state()?;
    let value = authenticated_post_json(
        &mut state,
        &path,
        &json!({
            "name": args.name,
            "contact_type": args.contact_type,
            "rfc": args.rfc,
            "email": args.email,
            "phone": args.phone,
            "notes": args.notes,
        }),
    )
    .await?;
    save_state(&state)?;
    print_ok_output(&value, json_output, "contact updated")
}

async fn forecast_create(args: ForecastWriteArgs, json_output: bool) -> Result<()> {
    let payload = forecast_payload(args)?;
    let mut state = load_state()?;
    let value = authenticated_post_json(&mut state, "/api/admin/forecasts", &payload).await?;
    save_state(&state)?;
    print_created_output(&value, json_output, "forecast")
}

async fn forecast_update(args: ForecastUpdateArgs, json_output: bool) -> Result<()> {
    validate_object_id(&args.id, "id")?;
    let payload = forecast_payload(args.fields)?;
    let path = format!("/api/admin/forecasts/{}/update", args.id);
    let mut state = load_state()?;
    let value = authenticated_post_json(&mut state, &path, &payload).await?;
    save_state(&state)?;
    print_ok_output(&value, json_output, "forecast updated")
}

async fn recurring_plan_create(args: RecurringPlanWriteArgs, json_output: bool) -> Result<()> {
    let payload = recurring_plan_payload(args)?;
    let mut state = load_state()?;
    let value = authenticated_post_json(&mut state, "/api/admin/recurring-plans", &payload).await?;
    save_state(&state)?;
    print_created_output(&value, json_output, "recurring plan")
}

async fn recurring_plan_update(args: RecurringPlanUpdateArgs, json_output: bool) -> Result<()> {
    validate_object_id(&args.id, "id")?;
    let payload = recurring_plan_payload(args.fields)?;
    let path = format!("/api/admin/recurring-plans/{}/update", args.id);
    let mut state = load_state()?;
    let value = authenticated_post_json(&mut state, &path, &payload).await?;
    save_state(&state)?;
    print_ok_output(&value, json_output, "recurring plan updated")
}

async fn recurring_plan_generate(id: String, json_output: bool) -> Result<()> {
    validate_object_id(&id, "id")?;
    let path = format!("/api/admin/recurring-plans/{id}/generate");
    let mut state = load_state()?;
    let value = authenticated_post_json(&mut state, &path, &json!({})).await?;
    save_state(&state)?;
    print_ok_output(&value, json_output, "recurring plan generated")
}

fn recurring_plan_payload(args: RecurringPlanWriteArgs) -> Result<Value> {
    validate_non_empty(&args.name, "name")?;
    validate_flow_type(&args.flow_type)?;
    validate_frequency(&args.frequency)?;
    validate_object_id(&args.category_id, "category-id")?;
    validate_object_id(&args.account_expected_id, "account-expected-id")?;
    validate_optional_object_id(args.contact_id.as_deref(), "contact-id")?;
    if args.amount_estimated < 0.0 {
        bail!("amount-estimated must be greater than or equal to zero");
    }
    if let Some(day) = args.day_of_month {
        if !(1..=31).contains(&day) {
            bail!("day-of-month must be between 1 and 31");
        }
    }
    let start_date = validate_rfc3339(&args.start_date, "start-date")?;
    let end_date = match args.end_date.as_deref() {
        Some(value) => Some(validate_rfc3339(value, "end-date")?),
        None => None,
    };
    Ok(json!({
        "name": args.name,
        "flow_type": args.flow_type,
        "category_id": args.category_id,
        "account_expected_id": args.account_expected_id,
        "contact_id": args.contact_id,
        "amount_estimated": args.amount_estimated,
        "frequency": args.frequency,
        "day_of_month": args.day_of_month,
        "start_date": start_date,
        "end_date": end_date,
        "is_active": !args.inactive,
        "version": args.version,
        "notes": args.notes,
    }))
}

async fn planned_entry_create(args: PlannedEntryWriteArgs, json_output: bool) -> Result<()> {
    let payload = planned_entry_payload(args)?;
    let mut state = load_state()?;
    let value = authenticated_post_json(&mut state, "/api/admin/planned-entries", &payload).await?;
    save_state(&state)?;
    print_created_output(&value, json_output, "planned entry")
}

async fn planned_entry_update(args: PlannedEntryUpdateArgs, json_output: bool) -> Result<()> {
    validate_object_id(&args.id, "id")?;
    let payload = planned_entry_payload(args.fields)?;
    let path = format!("/api/admin/planned-entries/{}/update", args.id);
    let mut state = load_state()?;
    let value = authenticated_post_json(&mut state, &path, &payload).await?;
    save_state(&state)?;
    print_ok_output(&value, json_output, "planned entry updated")
}

async fn planned_entry_pay(args: PlannedEntryPayArgs, json_output: bool) -> Result<()> {
    validate_object_id(&args.id, "id")?;
    validate_object_id(&args.account_id, "account-id")?;
    validate_optional_object_id(args.project_id.as_deref(), "project-id")?;
    validate_optional_object_id(
        args.parent_planned_entry_id.as_deref(),
        "parent-planned-entry-id",
    )?;
    if args.amount < 0.0 {
        bail!("amount must be greater than or equal to zero");
    }
    let paid_at = validate_rfc3339(&args.paid_at, "paid-at")?;
    let payload = json!({
        "paid_at": paid_at,
        "amount": args.amount,
        "account_id": args.account_id,
        "project_id": args.project_id,
        "parent_planned_entry_id": args.parent_planned_entry_id,
        "notes": args.notes,
    });
    let path = format!("/api/admin/planned-entries/{}/pay", args.id);
    let mut state = load_state()?;
    let value = authenticated_post_json(&mut state, &path, &payload).await?;
    save_state(&state)?;
    print_ok_output(&value, json_output, "planned entry paid")
}

async fn planned_entries_bulk_pay(args: PlannedEntryBulkPayArgs, json_output: bool) -> Result<()> {
    for id in &args.entry_ids {
        validate_object_id(id, "entry-id")?;
    }
    validate_object_id(&args.account_id, "account-id")?;
    validate_optional_object_id(args.project_id.as_deref(), "project-id")?;
    validate_optional_object_id(
        args.parent_planned_entry_id.as_deref(),
        "parent-planned-entry-id",
    )?;
    let paid_at = validate_rfc3339(&args.paid_at, "paid-at")?;
    let payload = json!({
        "entry_ids": args.entry_ids,
        "paid_at": paid_at,
        "account_id": args.account_id,
        "project_id": args.project_id,
        "parent_planned_entry_id": args.parent_planned_entry_id,
        "notes": args.notes,
    });
    let mut state = load_state()?;
    let value =
        authenticated_post_json(&mut state, "/api/admin/planned-entries/bulk-pay", &payload)
            .await?;
    save_state(&state)?;
    print_ok_output(&value, json_output, "planned entries paid")
}

fn planned_entry_payload(args: PlannedEntryWriteArgs) -> Result<Value> {
    validate_non_empty(&args.name, "name")?;
    validate_flow_type(&args.flow_type)?;
    validate_planned_status(&args.status)?;
    validate_object_id(&args.category_id, "category-id")?;
    validate_object_id(&args.account_expected_id, "account-expected-id")?;
    validate_optional_object_id(args.contact_id.as_deref(), "contact-id")?;
    validate_optional_object_id(args.project_id.as_deref(), "project-id")?;
    validate_optional_object_id(args.recurring_plan_id.as_deref(), "recurring-plan-id")?;
    if args.amount_estimated < 0.0 {
        bail!("amount-estimated must be greater than or equal to zero");
    }
    let due_date = validate_rfc3339(&args.due_date, "due-date")?;
    Ok(json!({
        "name": args.name,
        "flow_type": args.flow_type,
        "category_id": args.category_id,
        "account_expected_id": args.account_expected_id,
        "contact_id": args.contact_id,
        "project_id": args.project_id,
        "amount_estimated": args.amount_estimated,
        "due_date": due_date,
        "status": args.status,
        "recurring_plan_id": args.recurring_plan_id,
        "recurring_plan_version": args.recurring_plan_version,
        "notes": args.notes,
    }))
}

async fn transaction_create(args: TransactionWriteArgs, json_output: bool) -> Result<()> {
    let payload = transaction_payload(args)?;
    let mut state = load_state()?;
    let value = authenticated_post_json(&mut state, "/api/admin/transactions", &payload).await?;
    save_state(&state)?;
    print_created_output(&value, json_output, "transaction")
}

async fn transaction_update(args: TransactionUpdateArgs, json_output: bool) -> Result<()> {
    validate_object_id(&args.id, "id")?;
    let payload = transaction_payload(args.fields)?;
    let path = format!("/api/admin/transactions/{}/update", args.id);
    let mut state = load_state()?;
    let value = authenticated_post_json(&mut state, &path, &payload).await?;
    save_state(&state)?;
    print_ok_output(&value, json_output, "transaction updated")
}

fn transaction_payload(args: TransactionWriteArgs) -> Result<Value> {
    validate_non_empty(&args.description, "description")?;
    validate_transaction_type(&args.transaction_type)?;
    validate_object_id(&args.category_id, "category-id")?;
    validate_optional_object_id(args.account_from_id.as_deref(), "account-from-id")?;
    validate_optional_object_id(args.account_to_id.as_deref(), "account-to-id")?;
    validate_optional_object_id(args.planned_entry_id.as_deref(), "planned-entry-id")?;
    if args.amount < 0.0 {
        bail!("amount must be greater than or equal to zero");
    }
    let date = validate_rfc3339(&args.date, "date")?;
    Ok(json!({
        "date": date,
        "description": args.description,
        "transaction_type": args.transaction_type,
        "category_id": args.category_id,
        "account_from_id": args.account_from_id,
        "account_to_id": args.account_to_id,
        "amount": args.amount,
        "planned_entry_id": args.planned_entry_id,
        "is_confirmed": !args.unconfirmed,
        "notes": args.notes,
    }))
}

fn forecast_payload(args: ForecastWriteArgs) -> Result<Value> {
    validate_non_empty(&args.currency, "currency")?;
    let generated_at = normalize_timeline_bound(&args.generated_at, "generated-at")?;
    let start_date = normalize_timeline_bound(&args.start_date, "start-date")?;
    let end_date = normalize_timeline_bound(&args.end_date, "end-date")?;
    if let Some(user_id) = args.generated_by_user_id.as_deref() {
        validate_object_id(user_id, "generated-by-user-id")?;
    }
    Ok(json!({
        "generated_at": generated_at,
        "generated_by_user_id": args.generated_by_user_id,
        "start_date": start_date,
        "end_date": end_date,
        "currency": args.currency,
        "projected_income_total": args.projected_income_total,
        "projected_expense_total": args.projected_expense_total,
        "projected_net": args.projected_net,
        "initial_balance": args.initial_balance,
        "final_balance": args.final_balance,
        "details": args.details,
        "scenario_name": args.scenario_name,
        "notes": args.notes,
    }))
}

async fn delete_command(
    base_path: &str,
    args: DeleteArgs,
    json_output: bool,
    label: &str,
) -> Result<()> {
    if !args.yes {
        bail!("{label} delete is destructive; rerun with --yes to confirm");
    }
    validate_object_id(&args.id, "id")?;
    let path = format!("{base_path}/{}/delete", args.id);
    let mut state = load_state()?;
    let value = authenticated_post_json(&mut state, &path, &json!({})).await?;
    save_state(&state)?;
    print_ok_output(&value, json_output, &format!("{label} deleted"))
}

async fn project_status_create(args: ProjectStatusWriteArgs, json_output: bool) -> Result<()> {
    let payload = project_status_payload(args)?;
    let mut state = load_state()?;
    let value =
        authenticated_post_json(&mut state, "/api/admin/concept_statuses", &payload).await?;
    save_state(&state)?;
    print_created_output(&value, json_output, "concept status")
}

async fn project_status_update(args: ProjectStatusUpdateArgs, json_output: bool) -> Result<()> {
    validate_object_id(&args.id, "id")?;
    let payload = project_status_payload(args.fields)?;
    let path = format!("/api/admin/concept_statuses/{}/update", args.id);
    let mut state = load_state()?;
    let value = authenticated_post_json(&mut state, &path, &payload).await?;
    save_state(&state)?;
    print_ok_output(&value, json_output, "concept status updated")
}

fn project_status_payload(args: ProjectStatusWriteArgs) -> Result<Value> {
    validate_non_empty(&args.name, "name")?;
    Ok(json!({
        "name": args.name,
        "position": args.position,
        "color": args.color,
        "is_initial": args.initial,
        "is_terminal": args.terminal,
        "is_cancelled": args.cancelled,
        "is_active": !args.inactive,
    }))
}

async fn project_status_summary(args: ProjectStatusSummaryArgs, json_output: bool) -> Result<()> {
    validate_object_id(&args.project_id, "project-id")?;
    let path = format!("/api/admin/projects/{}/status_summary", args.project_id);
    json_get_command(&path, json_output, "project status summary").await
}

async fn project_concepts_list(args: ProjectConceptsListArgs, json_output: bool) -> Result<()> {
    validate_object_id(&args.project_id, "project-id")?;
    let path = format!("/api/admin/projects/{}/concepts", args.project_id);
    json_get_command(&path, json_output, "project concepts").await
}

async fn project_concept_create(args: ProjectConceptCreateArgs, json_output: bool) -> Result<()> {
    validate_object_id(&args.project_id, "project-id")?;
    let payload = project_concept_payload(args.fields)?;
    let path = format!("/api/admin/projects/{}/concepts", args.project_id);
    let mut state = load_state()?;
    let value = authenticated_post_json(&mut state, &path, &payload).await?;
    save_state(&state)?;
    print_created_output(&value, json_output, "project concept")
}

async fn project_concept_update(args: ProjectConceptUpdateArgs, json_output: bool) -> Result<()> {
    validate_object_id(&args.id, "id")?;
    let payload = project_concept_payload(args.fields)?;
    let path = format!("/api/admin/project_concepts/{}/update", args.id);
    let mut state = load_state()?;
    let value = authenticated_post_json(&mut state, &path, &payload).await?;
    save_state(&state)?;
    print_ok_output(&value, json_output, "project concept updated")
}

async fn project_concept_advance(id: &str, json_output: bool) -> Result<()> {
    validate_object_id(id, "id")?;
    let path = format!("/api/admin/project_concepts/{id}/advance");
    let mut state = load_state()?;
    let value = authenticated_post_json(&mut state, &path, &json!({})).await?;
    save_state(&state)?;
    print_value_output(&value, json_output, "project concept advanced")
}

fn project_concept_payload(args: ProjectConceptWriteArgs) -> Result<Value> {
    validate_non_empty(&args.name, "name")?;
    if args.quantity <= 0.0 {
        bail!("quantity must be greater than zero");
    }
    if let Some(status_id) = args.status_id.as_deref() {
        validate_object_id(status_id, "status-id")?;
    }
    Ok(json!({
        "status_id": args.status_id,
        "name": args.name,
        "quantity": args.quantity,
        "unit": args.unit,
        "description": args.description,
        "estimated_hours": args.estimated_hours,
        "estimated_cost": args.estimated_cost,
        "notes": args.notes,
        "position": args.position,
    }))
}

async fn cfdi_get(uuid: &str, json_output: bool) -> Result<()> {
    validate_non_empty(uuid, "uuid")?;
    let path = format!("/api/admin/cfdis/{}", uuid.trim());
    json_get_command(&path, json_output, "CFDI").await
}

async fn resource_usage_create(args: ResourceUsageCreateArgs, json_output: bool) -> Result<()> {
    validate_object_id(&args.resource_id, "resource-id")?;
    let started_at = validate_rfc3339(&args.started_at, "started-at")?;
    let ended_at = match args.ended_at {
        Some(value) => Some(validate_rfc3339(&value, "ended-at")?),
        None => None,
    };
    let mut state = load_state()?;
    let value = authenticated_post_json(
        &mut state,
        "/api/admin/resource_usages",
        &json!({
            "resource_id": args.resource_id,
            "started_at": started_at,
            "ended_at": ended_at,
            "operator_name": args.operator_name,
            "notes": args.notes,
        }),
    )
    .await?;
    save_state(&state)?;
    print_created_output(&value, json_output, "resource usage")
}

async fn resource_usage_update(args: ResourceUsageUpdateArgs, json_output: bool) -> Result<()> {
    validate_object_id(&args.id, "id")?;
    if args.hourly_cost_snapshot < 0.0 {
        bail!("hourly-cost-snapshot must be greater than or equal to zero");
    }
    let started_at = validate_rfc3339(&args.started_at, "started-at")?;
    let ended_at = match args.ended_at {
        Some(value) => Some(validate_rfc3339(&value, "ended-at")?),
        None => None,
    };
    let path = format!("/api/admin/resource_usages/{}/update", args.id);
    let mut state = load_state()?;
    let value = authenticated_post_json(
        &mut state,
        &path,
        &json!({
            "started_at": started_at,
            "ended_at": ended_at,
            "hourly_cost_snapshot": args.hourly_cost_snapshot,
            "operator_name": args.operator_name,
            "notes": args.notes,
        }),
    )
    .await?;
    save_state(&state)?;
    print_ok_output(&value, json_output, "resource usage updated")
}

async fn resource_usage_allocations_list(usage_id: &str, json_output: bool) -> Result<()> {
    validate_object_id(usage_id, "usage-id")?;
    let path = format!("/api/admin/resource_usages/{usage_id}/allocations");
    json_get_command(&path, json_output, "resource usage allocations").await
}

async fn resource_usage_allocations_replace(
    args: ResourceUsageAllocationsReplaceArgs,
    json_output: bool,
) -> Result<()> {
    validate_object_id(&args.usage_id, "usage-id")?;
    let payload = resource_usage_allocations_payload(args.concept_ids, args.allocations)?;
    let path = format!("/api/admin/resource_usages/{}/allocations", args.usage_id);
    let mut state = load_state()?;
    let value = authenticated_post_json(&mut state, &path, &payload).await?;
    save_state(&state)?;
    print_ok_output(&value, json_output, "resource usage allocations replaced")
}

fn resource_usage_allocations_payload(
    concept_ids: Vec<String>,
    allocations: Vec<String>,
) -> Result<Value> {
    if concept_ids.is_empty() == allocations.is_empty() {
        bail!("provide either --concept-id values or --allocation values");
    }
    if !concept_ids.is_empty() {
        for concept_id in &concept_ids {
            validate_object_id(concept_id, "concept-id")?;
        }
        return Ok(json!({ "concept_ids": concept_ids }));
    }

    let mut parsed = Vec::new();
    for allocation in allocations {
        let parts = allocation.splitn(3, ':').collect::<Vec<_>>();
        if parts.len() < 2 {
            bail!("allocation must be concept_id:ratio[:notes]");
        }
        validate_object_id(parts[0], "allocation concept-id")?;
        let ratio = parts[1]
            .parse::<f64>()
            .with_context(|| format!("allocation ratio is invalid: {allocation}"))?;
        if ratio <= 0.0 {
            bail!("allocation ratio must be greater than zero");
        }
        parsed.push(json!({
            "concept_id": parts[0],
            "ratio": ratio,
            "notes": parts.get(2).copied(),
        }));
    }
    Ok(json!({ "allocations": parsed }))
}

async fn cfdi_jobs_list(json_output: bool) -> Result<()> {
    let mut state = load_state()?;
    let company_id = selected_company_id(&mut state).await?;
    let path = format!("/admin/companies/{company_id}/cfdi/jobs");
    let value = authenticated_get(&mut state, &path).await?;
    save_state(&state)?;
    print_value_output(&value, json_output, "CFDI jobs")
}

async fn cfdi_job_status(job_id: &str, json_output: bool) -> Result<()> {
    validate_non_empty(job_id, "job-id")?;
    let mut state = load_state()?;
    let company_id = selected_company_id(&mut state).await?;
    let path = format!("/admin/companies/{company_id}/cfdi/jobs/{}", job_id.trim());
    let value = authenticated_get(&mut state, &path).await?;
    save_state(&state)?;
    print_value_output(&value, json_output, "CFDI job")
}

async fn selected_company_id(state: &mut CredentialState) -> Result<String> {
    let selected_slug = state
        .company_slug
        .clone()
        .ok_or_else(|| anyhow!("company selection is required; run spcli company use <slug>"))?;
    let companies: Vec<CompanySummary> =
        serde_json::from_value(authenticated_get(state, "/api/me/companies").await?)
            .context("failed to parse company list")?;
    let selected = companies
        .iter()
        .find(|company| company.slug.eq_ignore_ascii_case(&selected_slug))
        .ok_or_else(|| anyhow!("selected company '{selected_slug}' is no longer available"))?;
    validate_object_id(&selected.id, "selected company id")?;
    Ok(selected.id.clone())
}

async fn timeline(args: TimelineArgs, json_output: bool) -> Result<()> {
    validate_timeline_mode(&args.mode)?;
    let from = normalize_timeline_bound(&args.from, "from")?;
    let to = normalize_timeline_bound(&args.to, "to")?;
    let path = format!("/api/tiempo?mode={}&from={}&to={}", args.mode, from, to);
    json_get_command(&path, json_output, "timeline buckets").await
}

async fn pdf_preview(args: PdfPreviewArgs, json_output: bool) -> Result<()> {
    let source = read_pdf_source(args.input.as_ref(), args.source.as_deref())?;
    let mut state = load_state()?;
    let value =
        authenticated_post_json(&mut state, "/pdf/preview", &json!({ "source": source })).await?;
    save_state(&state)?;

    if json_output {
        print_json(&value)?;
        return Ok(());
    }

    if value["ok"].as_bool() != Some(true) {
        bail!(
            "PDF preview failed: {}",
            value["error"].as_str().unwrap_or("unknown error")
        );
    }

    let pdf_base64 = value["pdf_base64"]
        .as_str()
        .ok_or_else(|| anyhow!("PDF preview response did not include pdf_base64"))?;
    if let Some(output) = args.output {
        let bytes = data_encoding::BASE64
            .decode(pdf_base64.as_bytes())
            .map_err(|_| anyhow!("PDF preview response contained invalid base64"))?;
        fs::write(&output, bytes)
            .with_context(|| format!("failed to write {}", output.display()))?;
        println!("PDF written to {}", output.display());
    } else {
        println!("PDF preview succeeded ({} base64 bytes).", pdf_base64.len());
    }
    Ok(())
}

fn print_manifest(json_output: bool) -> Result<()> {
    let manifest = json!({
        "name": "spcli",
        "schema_version": "1",
        "commands": [
            { "name": "login", "auth_required": false, "company_required": false, "destructive": false },
            { "name": "status", "auth_required": true, "company_required": false, "destructive": false },
            { "name": "logout", "auth_required": false, "company_required": false, "destructive": false },
            { "name": "reset-auth", "auth_required": false, "company_required": false, "destructive": true, "confirmation_flag": "--yes" },
            { "name": "company list", "auth_required": true, "company_required": false, "destructive": false },
            { "name": "company use", "auth_required": true, "company_required": false, "destructive": false },
            { "name": "finance accounts list", "auth_required": true, "company_required": true, "destructive": false, "output_schema": "accounts" },
            { "name": "finance accounts get", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id"], "output_schema": "account" },
            { "name": "finance accounts create", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["--name", "--account-type", "--currency", "--inactive", "--notes"], "output_schema": "created_id" },
            { "name": "finance accounts update", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id", "--name", "--account-type", "--currency", "--inactive", "--notes"], "output_schema": "ok" },
            { "name": "finance accounts delete", "auth_required": true, "company_required": true, "destructive": true, "confirmation_flag": "--yes", "arguments": ["id"], "output_schema": "ok" },
            { "name": "finance categories list", "auth_required": true, "company_required": true, "destructive": false, "output_schema": "categories" },
            { "name": "finance categories get", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id"], "output_schema": "category" },
            { "name": "finance categories create", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["--name", "--flow-type", "--parent-id", "--notes"], "output_schema": "created_id" },
            { "name": "finance categories update", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id", "--name", "--flow-type", "--parent-id", "--notes"], "output_schema": "ok" },
            { "name": "finance categories delete", "auth_required": true, "company_required": true, "destructive": true, "confirmation_flag": "--yes", "arguments": ["id"], "output_schema": "ok" },
            { "name": "finance contacts list", "auth_required": true, "company_required": true, "destructive": false, "output_schema": "contacts" },
            { "name": "finance contacts get", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id"], "output_schema": "contact" },
            { "name": "finance contacts create", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["--name", "--contact-type", "--rfc", "--email", "--phone", "--notes"], "output_schema": "created_id" },
            { "name": "finance contacts update", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id", "--name", "--contact-type", "--rfc", "--email", "--phone", "--notes"], "output_schema": "ok" },
            { "name": "finance contacts delete", "auth_required": true, "company_required": true, "destructive": true, "confirmation_flag": "--yes", "arguments": ["id"], "output_schema": "ok" },
            { "name": "finance forecasts list", "auth_required": true, "company_required": true, "destructive": false, "output_schema": "forecasts" },
            { "name": "finance forecasts get", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id"], "output_schema": "forecast" },
            { "name": "finance forecasts create", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["--generated-at", "--start-date", "--end-date", "--currency", "--projected-income-total", "--projected-expense-total", "--projected-net"], "output_schema": "created_id" },
            { "name": "finance forecasts update", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id", "--generated-at", "--start-date", "--end-date", "--currency", "--projected-income-total", "--projected-expense-total", "--projected-net"], "output_schema": "ok" },
            { "name": "finance forecasts delete", "auth_required": true, "company_required": true, "destructive": true, "confirmation_flag": "--yes", "arguments": ["id"], "output_schema": "ok" },
            { "name": "finance recurring-plans list", "auth_required": true, "company_required": true, "destructive": false, "output_schema": "recurring_plans" },
            { "name": "finance recurring-plans get", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id"], "output_schema": "recurring_plan" },
            { "name": "finance recurring-plans create", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["--name", "--flow-type", "--category-id", "--account-expected-id", "--contact-id", "--amount-estimated", "--frequency", "--day-of-month", "--start-date", "--end-date", "--inactive", "--version", "--notes"], "output_schema": "created_id_with_side_effects" },
            { "name": "finance recurring-plans update", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id", "--name", "--flow-type", "--category-id", "--account-expected-id", "--contact-id", "--amount-estimated", "--frequency", "--day-of-month", "--start-date", "--end-date", "--inactive", "--version", "--notes"], "output_schema": "ok_with_side_effects" },
            { "name": "finance recurring-plans delete", "auth_required": true, "company_required": true, "destructive": true, "confirmation_flag": "--yes", "arguments": ["id"], "output_schema": "ok_with_side_effects" },
            { "name": "finance recurring-plans generate", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id"], "output_schema": "ok_with_side_effects" },
            { "name": "finance planned-entries list", "auth_required": true, "company_required": true, "destructive": false, "output_schema": "planned_entries" },
            { "name": "finance planned-entries get", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id"], "output_schema": "planned_entry" },
            { "name": "finance planned-entries create", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["--name", "--flow-type", "--category-id", "--account-expected-id", "--contact-id", "--project-id", "--amount-estimated", "--due-date", "--status", "--recurring-plan-id", "--recurring-plan-version", "--notes"], "output_schema": "created_id_with_side_effects" },
            { "name": "finance planned-entries update", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id", "--name", "--flow-type", "--category-id", "--account-expected-id", "--contact-id", "--project-id", "--amount-estimated", "--due-date", "--status", "--recurring-plan-id", "--recurring-plan-version", "--notes"], "output_schema": "ok_with_side_effects" },
            { "name": "finance planned-entries delete", "auth_required": true, "company_required": true, "destructive": true, "confirmation_flag": "--yes", "arguments": ["id"], "output_schema": "ok" },
            { "name": "finance planned-entries pay", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id", "--paid-at", "--amount", "--account-id", "--project-id", "--parent-planned-entry-id", "--notes"], "output_schema": "ok_with_side_effects" },
            { "name": "finance planned-entries bulk-pay", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["--entry-id", "--paid-at", "--account-id", "--project-id", "--parent-planned-entry-id", "--notes"], "output_schema": "ok_with_side_effects" },
            { "name": "finance transactions list", "auth_required": true, "company_required": true, "destructive": false, "output_schema": "transactions" },
            { "name": "finance transactions get", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id"], "output_schema": "transaction" },
            { "name": "finance transactions create", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["--date", "--description", "--transaction-type", "--category-id", "--account-from-id", "--account-to-id", "--amount", "--planned-entry-id", "--unconfirmed", "--notes"], "output_schema": "created_id_with_side_effects" },
            { "name": "finance transactions update", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id", "--date", "--description", "--transaction-type", "--category-id", "--account-from-id", "--account-to-id", "--amount", "--planned-entry-id", "--unconfirmed", "--notes"], "output_schema": "ok_with_side_effects" },
            { "name": "finance transactions delete", "auth_required": true, "company_required": true, "destructive": true, "confirmation_flag": "--yes", "arguments": ["id"], "output_schema": "ok_with_side_effects" },
            { "name": "cfdi list", "auth_required": true, "company_required": true, "destructive": false, "output_schema": "cfdi_data" },
            { "name": "cfdi get", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["uuid"], "output_schema": "cfdi_detail" },
            { "name": "cfdi jobs list", "auth_required": true, "company_required": true, "destructive": false, "output_schema": "cfdi_jobs" },
            { "name": "cfdi jobs status", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["job-id"], "output_schema": "cfdi_job" },
            { "name": "sat configs list", "auth_required": true, "company_required": true, "destructive": false, "output_schema": "sat_configs" },
            { "name": "sat configs get", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id"], "output_schema": "sat_config" },
            { "name": "projects list", "auth_required": true, "company_required": true, "destructive": false, "output_schema": "projects" },
            { "name": "projects get", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id"], "output_schema": "project" },
            { "name": "projects status-summary", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["--project-id"], "output_schema": "project_status_summary" },
            { "name": "projects statuses list", "auth_required": true, "company_required": true, "destructive": false, "output_schema": "concept_statuses" },
            { "name": "projects statuses create", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["--name", "--position", "--color", "--initial", "--terminal", "--cancelled", "--inactive"], "output_schema": "created_id" },
            { "name": "projects statuses update", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id", "--name", "--position", "--color", "--initial", "--terminal", "--cancelled", "--inactive"], "output_schema": "ok" },
            { "name": "projects statuses delete", "auth_required": true, "company_required": true, "destructive": true, "confirmation_flag": "--yes", "arguments": ["id"], "output_schema": "ok" },
            { "name": "projects concepts list", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["--project-id"], "output_schema": "project_concepts" },
            { "name": "projects concepts create", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["--project-id", "--name", "--quantity", "--status-id", "--unit", "--description", "--estimated-hours", "--estimated-cost", "--notes", "--position"], "output_schema": "created_id" },
            { "name": "projects concepts update", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id", "--name", "--quantity", "--status-id", "--unit", "--description", "--estimated-hours", "--estimated-cost", "--notes", "--position"], "output_schema": "ok" },
            { "name": "projects concepts advance", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id"], "output_schema": "concept_status" },
            { "name": "projects concepts delete", "auth_required": true, "company_required": true, "destructive": true, "confirmation_flag": "--yes", "arguments": ["id"], "output_schema": "ok" },
            { "name": "resources list", "auth_required": true, "company_required": true, "destructive": false, "output_schema": "resources" },
            { "name": "resources get", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id"], "output_schema": "resource" },
            { "name": "resources logs list", "auth_required": true, "company_required": true, "destructive": false, "output_schema": "resource_logs" },
            { "name": "resources logs get", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id"], "output_schema": "resource_log" },
            { "name": "resources usages list", "auth_required": true, "company_required": true, "destructive": false, "output_schema": "resource_usages" },
            { "name": "resources usages get", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id"], "output_schema": "resource_usage" },
            { "name": "resources usages create", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["--resource-id", "--started-at", "--ended-at", "--operator-name", "--notes"], "output_schema": "created_id" },
            { "name": "resources usages update", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["id", "--started-at", "--ended-at", "--hourly-cost-snapshot", "--operator-name", "--notes"], "output_schema": "ok" },
            { "name": "resources usages delete", "auth_required": true, "company_required": true, "destructive": true, "confirmation_flag": "--yes", "arguments": ["id"], "output_schema": "ok" },
            { "name": "resources usages allocations list", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["usage-id"], "output_schema": "resource_usage_allocations" },
            { "name": "resources usages allocations replace", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["usage-id", "--concept-id", "--allocation"], "output_schema": "allocation_ids" },
            { "name": "time timeline", "auth_required": true, "company_required": true, "destructive": false, "arguments": ["--mode", "--from", "--to"], "output_schema": "timeline_buckets" },
            { "name": "pdf preview", "auth_required": true, "company_required": false, "destructive": false, "arguments": ["--input", "--source", "--output"], "output_schema": "pdf_preview" },
            { "name": "manifest", "auth_required": false, "company_required": false, "destructive": false }
        ],
        "output": { "human_default": true, "json_flag": "--json" }
    });
    if json_output {
        print_json(&manifest)?;
    } else {
        println!(
            "spcli commands: login, status, logout, reset-auth, company, finance, cfdi, sat, projects, resources, time, pdf, manifest"
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

async fn authenticated_post_json<T: Serialize>(
    state: &mut CredentialState,
    path: &str,
    payload: &T,
) -> Result<Value> {
    ensure_session(state).await?;
    let response = post_json_with_session(state, path, payload).await?;
    if response.status() == StatusCode::UNAUTHORIZED {
        refresh_login(state).await?;
        let retry = post_json_with_session(state, path, payload).await?;
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

async fn post_json_with_session<T: Serialize>(
    state: &CredentialState,
    path: &str,
    payload: &T,
) -> Result<reqwest::Response> {
    let client = Client::new();
    let url = endpoint(&request_base_url(state)?, path)?;
    let mut request = client
        .post(url)
        .header(header::COOKIE, session_cookie_header(state)?)
        .json(payload);
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

fn print_value_output(value: &Value, json_output: bool, label: &str) -> Result<()> {
    if json_output {
        print_json(value)?;
        return Ok(());
    }

    if let Some(items) = value.as_array() {
        println!("{}: {}", label, items.len());
    } else if let Some(items) = value.get("items").and_then(Value::as_array) {
        println!("{}: {}", label, items.len());
    } else if let Some(object) = value.as_object() {
        println!("{} fields: {}", label, object.len());
    } else {
        println!("{} returned.", label);
    }
    Ok(())
}

fn print_created_output(value: &Value, json_output: bool, label: &str) -> Result<()> {
    if json_output {
        print_json(value)?;
    } else {
        println!(
            "Created {} {}",
            label,
            value["id"].as_str().unwrap_or("unknown")
        );
    }
    Ok(())
}

fn print_ok_output(value: &Value, json_output: bool, message: &str) -> Result<()> {
    if json_output {
        print_json(value)?;
    } else {
        println!("{message}");
    }
    Ok(())
}

fn validate_non_empty(value: &str, name: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{name} is required")
    } else {
        Ok(())
    }
}

fn validate_object_id(value: &str, name: &str) -> Result<()> {
    if value.len() == 24 && value.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(())
    } else {
        bail!("{name} must be a 24-character ObjectId")
    }
}

fn validate_optional_object_id(value: Option<&str>, name: &str) -> Result<()> {
    if let Some(value) = value {
        validate_object_id(value, name)?;
    }
    Ok(())
}

fn validate_transaction_type(value: &str) -> Result<()> {
    match value {
        "income" | "expense" | "transfer" => Ok(()),
        _ => bail!("transaction-type must be one of: income, expense, transfer"),
    }
}

fn validate_flow_type(value: &str) -> Result<()> {
    match value {
        "income" | "expense" => Ok(()),
        _ => bail!("flow-type must be one of: income, expense"),
    }
}

fn validate_frequency(value: &str) -> Result<()> {
    match value {
        "monthly" | "weekly" => Ok(()),
        _ => bail!("frequency must be one of: monthly, weekly"),
    }
}

fn validate_planned_status(value: &str) -> Result<()> {
    match value {
        "planned" | "partially_covered" | "covered" | "overdue" | "cancelled" => Ok(()),
        _ => {
            bail!("status must be one of: planned, partially_covered, covered, overdue, cancelled")
        }
    }
}

fn validate_timeline_mode(value: &str) -> Result<()> {
    match value {
        "day" | "week" | "month" | "year" => Ok(()),
        _ => bail!("mode must be one of: day, week, month, year"),
    }
}

fn normalize_timeline_bound(value: &str, name: &str) -> Result<String> {
    if chrono::DateTime::parse_from_rfc3339(value).is_ok() {
        return Ok(value.to_string());
    }
    let date = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .with_context(|| format!("{name} must be YYYY-MM-DD or RFC3339"))?;
    let datetime = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("{name} is out of range"))?;
    Ok(format!("{}Z", datetime.format("%Y-%m-%dT%H:%M:%S")))
}

fn validate_rfc3339(value: &str, name: &str) -> Result<String> {
    chrono::DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("{name} must be RFC3339"))?;
    Ok(value.to_string())
}

fn read_pdf_source(input: Option<&PathBuf>, source: Option<&str>) -> Result<String> {
    if let Some(source) = source {
        return Ok(source.to_string());
    }
    if let Some(input) = input {
        return fs::read_to_string(input)
            .with_context(|| format!("failed to read {}", input.display()));
    }
    let mut source = String::new();
    std::io::stdin()
        .read_to_string(&mut source)
        .context("failed to read Typst source from stdin")?;
    if source.trim().is_empty() {
        bail!("Typst source is required via --source, --input, or stdin");
    }
    Ok(source)
}

fn classify_error(err: &anyhow::Error) -> &'static str {
    let message = err.to_string().to_lowercase();
    if message.contains("credential file not found") || message.contains("missing session cookie") {
        "not_authenticated"
    } else if message.contains("login failed") {
        "invalid_credentials"
    } else if message.contains("unauthorized") || message.contains("status 401") {
        "unauthorized"
    } else if message.contains("forbidden") || message.contains("status 403") {
        "forbidden"
    } else if message.contains("not found") || message.contains("status 404") {
        "not_found"
    } else if message.contains("conflict") || message.contains("status 409") {
        "conflict"
    } else if message.contains("--yes") || message.contains("confirm") {
        "confirmation_required"
    } else if message.contains("must be")
        || message.contains("invalid")
        || message.contains("required")
        || message.contains("unsupported")
    {
        "validation_error"
    } else if message.contains("request failed") || message.contains("network") {
        "network_error"
    } else if message.contains("status 5") || message.contains("server") {
        "server_error"
    } else {
        "spcli_error"
    }
}

fn exit_code(code: &str) -> u8 {
    match code {
        "validation_error" | "confirmation_required" => 2,
        "not_authenticated" | "unauthorized" | "invalid_credentials" => 3,
        "forbidden" => 4,
        "not_found" => 5,
        "conflict" => 6,
        "network_error" | "server_error" => 7,
        _ => 1,
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
