mod common;

use common::{DockerEnvironment, TestEnvironment};
use pretty_assertions::assert_eq;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn test_search_recipes_no_query() {
    common::init_test_logging();
    DockerEnvironment::ensure_running().expect("Docker environment not running");

    let env = TestEnvironment::new()
        .await
        .expect("Failed to create test environment");

    // Search without query should return all recipes
    let result = env.client.search_recipes(None, Some(5), None).await;

    assert!(result.is_ok(), "Search should succeed");
    let response = result.unwrap();
    assert!(response.results.len() <= 5, "Should respect limit");
}

#[tokio::test]
#[serial]
async fn test_search_recipes_with_query() {
    common::init_test_logging();
    DockerEnvironment::ensure_running().expect("Docker environment not running");

    let env = TestEnvironment::new()
        .await
        .expect("Failed to create test environment");

    // Search with a specific query
    let result = env
        .client
        .search_recipes(Some("pasta"), Some(5), None)
        .await;

    assert!(result.is_ok(), "Search should succeed");
    let response = result.unwrap();
    assert!(response.results.len() <= 5, "Should respect limit");
}

#[tokio::test]
#[serial]
async fn test_create_and_retrieve_recipe() {
    common::init_test_logging();
    DockerEnvironment::ensure_running().expect("Docker environment not running");

    let env = TestEnvironment::new()
        .await
        .expect("Failed to create test environment");

    // Create a test recipe
    let test_name = format!("Test Recipe {}", chrono::Utc::now().timestamp());
    let create_request = mcp_tandoor::client::types::CreateRecipeRequest {
        name: test_name.clone(),
        description: Some("A test recipe created by integration tests".to_string()),
        servings: Some(4),
        working_time: 30,
        waiting_time: 15,
        keywords: vec![mcp_tandoor::client::types::CreateKeywordRequest {
            name: "test".to_string(),
        }],
        steps: vec![
            mcp_tandoor::client::types::CreateStepRequest {
                name: Some("Preparation".to_string()),
                instruction: "Prepare all ingredients".to_string(),
                ingredients: vec![],
                time: Some(10),
                order: 1,
            },
            mcp_tandoor::client::types::CreateStepRequest {
                name: Some("Cooking".to_string()),
                instruction: "Cook everything together".to_string(),
                ingredients: vec![],
                time: Some(20),
                order: 2,
            },
        ],
    };

    let create_result = env.client.create_recipe(create_request).await;
    assert!(create_result.is_ok(), "Recipe creation should succeed");

    let created_recipe = create_result.unwrap();
    assert_eq!(created_recipe.name, test_name, "Recipe name should match");
    assert_eq!(created_recipe.servings, Some(4), "Servings should match");

    // Retrieve the created recipe
    let get_result = env.client.get_recipe(created_recipe.id).await;
    assert!(get_result.is_ok(), "Should retrieve recipe successfully");

    let retrieved_recipe = get_result.unwrap();
    assert_eq!(
        retrieved_recipe.id, created_recipe.id,
        "Recipe ID should match"
    );
    assert_eq!(retrieved_recipe.name, test_name, "Recipe name should match");
    assert_eq!(retrieved_recipe.steps.len(), 2, "Should have 2 steps");
}

#[tokio::test]
#[serial]
async fn test_get_recipe_not_found() {
    common::init_test_logging();
    DockerEnvironment::ensure_running().expect("Docker environment not running");

    let env = TestEnvironment::new()
        .await
        .expect("Failed to create test environment");

    // Try to get a recipe with an ID that doesn't exist
    let result = env.client.get_recipe(999999).await;

    assert!(result.is_err(), "Should fail for non-existent recipe");
}
