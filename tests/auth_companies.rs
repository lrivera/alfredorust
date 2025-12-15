#[path = "common/mod.rs"]
mod common;

use alfredodev::models::{AccountType, UserRole};
use alfredodev::state::{
    add_user_to_company, create_account, create_company, create_session, create_user,
    delete_account, delete_company, delete_session, delete_user, find_user_by_session,
    get_company_by_id, get_user_by_id, list_companies, list_users, update_company, update_user,
};

#[tokio::test]
async fn users_crud_and_memberships_work() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();

    let companies = list_companies(&state).await.unwrap();
    let primary = companies[0].id.clone().unwrap();
    let extra_company =
        create_company(&state, "Test Org Extra", "", "", true, None).await.unwrap();

    let user_id = create_user(
        &state,
        "tester@example.com",
        "secret123",
        &[
            (primary.clone(), UserRole::Admin),
            (extra_company.clone(), UserRole::Staff),
        ],
    )
    .await
    .unwrap();

    let created = get_user_by_id(&state, &user_id).await.unwrap().unwrap();
    assert_eq!(created.email, "tester@example.com");
    assert_eq!(created.role, UserRole::Admin);
    assert!(created.company_ids.contains(&extra_company));
    assert_eq!(created.company_roles.len(), 2);

    // Update to single company and verify role change
    update_user(
        &state,
        &user_id,
        "tester+updated@example.com",
        "newsecret",
        &[(primary.clone(), UserRole::Staff)],
    )
    .await
    .unwrap();
    let updated = get_user_by_id(&state, &user_id).await.unwrap().unwrap();
    assert_eq!(updated.email, "tester+updated@example.com");
    assert_eq!(updated.company_roles.len(), 1);
    assert_eq!(updated.role, UserRole::Staff);

    // Add back to another company through membership helper
    add_user_to_company(&state, &user_id, &extra_company, UserRole::Admin)
        .await
        .unwrap();
    let with_membership = get_user_by_id(&state, &user_id).await.unwrap().unwrap();
    assert!(with_membership.company_ids.contains(&extra_company));
    assert_eq!(with_membership.company_roles.len(), 2);

    delete_user(&state, &user_id).await.unwrap();
    assert!(get_user_by_id(&state, &user_id).await.unwrap().is_none());

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn adding_memberships_reflects_in_user_companies() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();

    // Create a new company and a user tied to the first seeded company.
    let companies = list_companies(&state).await.unwrap();
    let first_company = companies[0].id.clone().unwrap();
    let second_company = create_company(&state, "Org Extra Test", "", "MXN", true, None)
        .await
        .unwrap();

    let user_id = create_user(
        &state,
        "membership@example.com",
        "secret",
        &[(first_company.clone(), UserRole::Admin)],
    )
    .await
    .unwrap();

    // Attach the user to the new company.
    add_user_to_company(&state, &user_id, &second_company, UserRole::Staff)
        .await
        .unwrap();

    // Fetch and confirm both companies are present.
    let user = get_user_by_id(&state, &user_id).await.unwrap().unwrap();
    assert_eq!(user.company_ids.len(), 2);
    assert!(user.company_ids.contains(&first_company));
    assert!(user.company_ids.contains(&second_company));
    assert_eq!(user.company_roles.len(), 2);

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn sessions_resolve_user() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();

    let user = list_users(&state).await.unwrap().remove(0);
    let token = create_session(&state, &user.email).await.unwrap();
    let fetched = find_user_by_session(&state, &token).await.unwrap();
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched.email, user.email);

    delete_session(&state, &token).await.unwrap();
    assert!(find_user_by_session(&state, &token)
        .await
        .unwrap()
        .is_none());

    common::teardown(Some(ctx)).await;
}

#[tokio::test]
async fn companies_crud_and_deletion_rules_work() {
    let ctx = match common::setup_state().await {
        Some(c) => c,
        None => return,
    };
    let state = ctx.state.clone();

    let company_id = create_company(&state, "Nueva Compania", "", "", true, None)
        .await
        .unwrap();
    let created = get_company_by_id(&state, &company_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(created.slug, "nueva-compania");
    assert_eq!(created.default_currency, "MXN");

    update_company(
        &state,
        &company_id,
        "Compania Renombrada",
        "slug-personal",
        "USD",
        false,
        Some("nota".to_string()),
    )
    .await
    .unwrap();
    let updated = get_company_by_id(&state, &company_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.slug, "slug-personal");
    assert_eq!(updated.default_currency, "USD");
    assert!(!updated.is_active);

    // Create a dependent record so delete becomes a soft-deactivation first.
    let acc = create_account(
        &state,
        &company_id,
        "Cuenta Temporal",
        AccountType::Bank,
        "",
        true,
        None,
    )
    .await
    .unwrap();

    delete_company(&state, &company_id).await.unwrap();
    let soft = get_company_by_id(&state, &company_id)
        .await
        .unwrap()
        .unwrap();
    assert!(!soft.is_active);

    // Remove dependency then hard-delete
    delete_account(&state, &acc).await.unwrap();
    delete_company(&state, &company_id).await.unwrap();
    assert!(get_company_by_id(&state, &company_id)
        .await
        .unwrap()
        .is_none());

    common::teardown(Some(ctx)).await;
}
