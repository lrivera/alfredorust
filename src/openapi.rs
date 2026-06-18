// openapi.rs
// Aggregated OpenAPI document for the JSON API, served through Swagger UI.
//
// The whole document is gated behind the session middleware (see main.rs), so
// the interactive docs are only reachable by a logged-in user. Authentication
// itself is a session cookie named `session`, modeled below as an apiKey scheme.

use utoipa::{
    Modify, OpenApi,
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
};

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "session",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new(
                crate::session::SESSION_COOKIE_NAME,
            ))),
        );
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "alfredodev API",
        version = "0.1.0",
        description = "JSON API for the multi-tenant financial management app. Every protected endpoint requires a valid session cookie."
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "auth", description = "Login, session, profile and current-user context"),
        (name = "finance", description = "Accounts, categories, contacts, recurring plans, planned entries, transactions, forecasts"),
        (name = "operations", description = "Service orders, projects, concept statuses and project concepts"),
        (name = "resources", description = "Resources, resource logs and resource usage tracking"),
        (name = "cfdi", description = "CFDI reads and SAT download jobs"),
        (name = "admin", description = "Company metadata, users and SAT configuration administration")
    ),
    paths(
        // auth / profile / misc
        crate::routes::login::login,
        crate::routes::logout::logout,
        crate::routes::setup::setup,
        crate::routes::secret::secret_generate,
        crate::routes::profile::me_companies,
        crate::routes::profile::me,
        crate::routes::tiempo::tiempo_data,
        crate::routes::pdf::pdf_preview,
        crate::routes::admin::account::account_profile_data_api,
        crate::routes::admin::account::account_profile_update_api,

        // finance — accounts / categories / contacts
        crate::routes::admin::finance::accounts::accounts_data_api,
        crate::routes::admin::finance::accounts::accounts_create_api,
        crate::routes::admin::finance::accounts::account_data_api,
        crate::routes::admin::finance::accounts::account_update_api,
        crate::routes::admin::finance::accounts::account_delete_api,
        crate::routes::admin::finance::categories::categories_data_api,
        crate::routes::admin::finance::categories::categories_create_api,
        crate::routes::admin::finance::categories::category_data_api,
        crate::routes::admin::finance::categories::category_update_api,
        crate::routes::admin::finance::categories::category_delete_api,
        crate::routes::admin::finance::contacts::contacts_data_api,
        crate::routes::admin::finance::contacts::contacts_create_api,
        crate::routes::admin::finance::contacts::contact_data_api,
        crate::routes::admin::finance::contacts::contact_update_api,
        crate::routes::admin::finance::contacts::contact_delete_api,

        // finance — recurring plans / planned entries
        crate::routes::admin::finance::recurring_plans::recurring_plans_data_api,
        crate::routes::admin::finance::recurring_plans::recurring_plans_create_api,
        crate::routes::admin::finance::recurring_plans::recurring_plan_data_api,
        crate::routes::admin::finance::recurring_plans::recurring_plan_update_api,
        crate::routes::admin::finance::recurring_plans::recurring_plan_delete_api,
        crate::routes::admin::finance::recurring_plans::recurring_plan_generate_api,
        crate::routes::admin::finance::planned_entries::planned_entries_data_api,
        crate::routes::admin::finance::planned_entries::planned_entries_create_api,
        crate::routes::admin::finance::planned_entries::planned_entries_bulk_pay_api,
        crate::routes::admin::finance::planned_entries::planned_entry_data_api,
        crate::routes::admin::finance::planned_entries::planned_entry_update_api,
        crate::routes::admin::finance::planned_entries::planned_entry_delete_api,
        crate::routes::admin::finance::planned_entries::planned_entry_pay_api,

        // finance — transactions / forecasts
        crate::routes::admin::finance::transactions::transactions_data_api,
        crate::routes::admin::finance::transactions::transactions_create_api,
        crate::routes::admin::finance::transactions::transaction_data_api,
        crate::routes::admin::finance::transactions::transaction_update_api,
        crate::routes::admin::finance::transactions::transaction_delete_api,
        crate::routes::admin::finance::forecasts::forecasts_data_api,
        crate::routes::admin::finance::forecasts::forecasts_create_api,
        crate::routes::admin::finance::forecasts::forecast_data_api,
        crate::routes::admin::finance::forecasts::forecast_update_api,
        crate::routes::admin::finance::forecasts::forecast_delete_api,

        // operations — orders
        crate::routes::admin::finance::orders::orders_data_api,
        crate::routes::admin::finance::orders::orders_create_api,
        crate::routes::admin::finance::orders::order_data_api,
        crate::routes::admin::finance::orders::order_update_api,
        crate::routes::admin::finance::orders::order_delete_api,
        crate::routes::admin::finance::orders::order_complete_api,

        // operations — projects
        crate::routes::admin::projects::projects_data_api,
        crate::routes::admin::projects::projects_create_api,
        crate::routes::admin::projects::project_data_api,
        crate::routes::admin::projects::project_update_api,
        crate::routes::admin::projects::project_delete_api,
        crate::routes::admin::projects::project_advance_api,

        // operations — concept statuses / project concepts (project_backend)
        crate::routes::admin::project_backend::api_concept_statuses_index,
        crate::routes::admin::project_backend::api_concept_statuses_create,
        crate::routes::admin::project_backend::api_concept_statuses_update,
        crate::routes::admin::project_backend::api_concept_statuses_delete,
        crate::routes::admin::project_backend::api_project_concepts_index,
        crate::routes::admin::project_backend::api_project_concepts_create,
        crate::routes::admin::project_backend::api_project_status_summary,
        crate::routes::admin::project_backend::api_project_concepts_update,
        crate::routes::admin::project_backend::api_project_concepts_advance,
        crate::routes::admin::project_backend::api_project_concepts_delete,

        // resources — usage grid + usages (project_backend)
        crate::routes::admin::project_backend::api_resource_usages_index,
        crate::routes::admin::project_backend::api_resource_usages_create,
        crate::routes::admin::project_backend::api_resource_usages_grid_save,
        crate::routes::admin::project_backend::api_resource_usages_grid_view,
        crate::routes::admin::project_backend::api_resource_usage_detail,
        crate::routes::admin::project_backend::api_resource_usages_update,
        crate::routes::admin::project_backend::api_resource_usages_delete,
        crate::routes::admin::project_backend::api_resource_usage_allocations_index,
        crate::routes::admin::project_backend::api_resource_usage_allocations_replace,

        // resources — resources / resource logs
        crate::routes::admin::resources::resources_data_api,
        crate::routes::admin::resources::resources_create_api,
        crate::routes::admin::resources::resource_data_api,
        crate::routes::admin::resources::resource_update_api,
        crate::routes::admin::resources::resource_delete_api,
        crate::routes::admin::resource_logs::resource_logs_data_api,
        crate::routes::admin::resource_logs::resource_logs_create_api,
        crate::routes::admin::resource_logs::resource_log_data_api,
        crate::routes::admin::resource_logs::resource_log_update_api,
        crate::routes::admin::resource_logs::resource_log_delete_api,
        crate::routes::admin::resource_logs::resource_log_end_api,

        // admin — companies / users
        crate::routes::admin::companies::companies_data_api,
        crate::routes::admin::companies::company_create_api,
        crate::routes::admin::companies::company_data_api,
        crate::routes::admin::companies::company_update_api,
        crate::routes::admin::users_api::api_users_index,
        crate::routes::admin::users_api::api_user_detail,
        crate::routes::admin::users_api::api_users_create,
        crate::routes::admin::users_api::api_users_update,
        crate::routes::admin::users_api::api_users_delete,

        // cfdi — reads / download jobs
        crate::routes::admin::cfdis::cfdis_data_api,
        crate::routes::admin::cfdis::cfdi_data_api,
        crate::routes::admin::cfdi_download::company_cfdi_download,
        crate::routes::admin::cfdi_download::company_cfdi_jobs_list,
        crate::routes::admin::cfdi_download::company_cfdi_job_status,

        // admin — SAT configs
        crate::routes::admin::sat_configs::sat_configs_data_api,
        crate::routes::admin::sat_configs::sat_config_create_api,
        crate::routes::admin::sat_configs::sat_config_upload_api,
        crate::routes::admin::sat_configs::sat_config_data_api,
        crate::routes::admin::sat_configs::sat_config_update_api,
        crate::routes::admin::sat_configs::sat_config_delete_api,
    )
)]
pub struct ApiDoc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_document_is_complete_and_secured() {
        let doc = ApiDoc::openapi();
        let json = serde_json::to_value(&doc).expect("OpenAPI doc must serialize");

        let paths = json["paths"].as_object().expect("paths object");
        assert!(
            paths.len() >= 80,
            "expected the full API surface to be documented, got {} paths",
            paths.len()
        );

        // The three newly added endpoints must be present.
        assert!(paths.contains_key("/api/admin/users"));
        assert!(paths.contains_key("/api/admin/users/{id}"));
        assert!(paths.contains_key("/api/admin/resource_usages/grid"));
        assert!(paths.contains_key("/api/admin/sat-configs/upload"));

        // Cookie-session security scheme is registered.
        assert!(
            json["components"]["securitySchemes"]["session"].is_object(),
            "session security scheme must be registered"
        );
    }
}
