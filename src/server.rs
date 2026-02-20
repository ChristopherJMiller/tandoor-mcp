//! # Tandoor MCP Server Implementation
//!
//! This module implements a Model Context Protocol (MCP) server that exposes Tandoor
//! functionality as standardized tools for AI assistants. The server handles authentication,
//! recipe management, shopping lists, meal planning, and more.
//!
//! ## Key Features
//!
//! - **Recipe Management**: Search, create, retrieve, and manage recipes
//! - **Shopping Lists**: Add items, manage shopping lists, check off items
//! - **Meal Planning**: Plan meals and add to shopping lists
//! - **Food & Ingredient Search**: Find foods and ingredients in the database
//! - **Import Capabilities**: Import recipes from URLs
//! - **Unit Management**: Get available measurement units
//! - **Keyword Management**: Get and search recipe keywords
//!
//! ## Authentication
//!
//! The server supports both credential-based authentication and pre-set tokens
//! to work around Tandoor's strict rate limiting (10 auth requests per day).

use rmcp::{
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::*,
    schemars,
    service::RequestContext,
    tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler,
};
use serde_json::json;
use std::future::Future;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::Mutex;

use crate::client::TandoorClient;

// Parameter structs for tools
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchRecipesParams {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub limit: Option<i32>,
    /// 1-indexed page number for pagination
    #[serde(default)]
    pub page: Option<i32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetRecipeDetailsParams {
    pub id: i32,
    #[serde(default)]
    pub servings: Option<i32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateRecipeParams {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub servings: Option<i32>,
    #[serde(default)]
    pub prep_time: Option<i32>,
    #[serde(default)]
    pub cook_time: Option<i32>,
    #[serde(default)]
    pub keywords: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ImportRecipeParams {
    pub url: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ShoppingItem {
    pub name: String,
    #[serde(default = "default_amount")]
    pub amount: f64,
    #[serde(default)]
    pub unit: Option<String>,
}

fn default_amount() -> f64 {
    1.0
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AddToShoppingListParams {
    #[serde(default)]
    pub items: Option<Vec<ShoppingItem>>,
    #[serde(default)]
    pub request: Option<String>,
    #[serde(default)]
    pub from_recipe: Option<AddFromRecipeParams>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AddFromRecipeParams {
    pub recipe_id: i32,
    #[serde(default)]
    pub servings: Option<i32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetShoppingListParams {
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "flat".to_string()
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CheckShoppingItemsParams {
    pub items: Vec<serde_json::Value>, // Can be strings (names) or numbers (IDs)
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchFoodsParams {
    pub query: String,
    #[serde(default)]
    pub limit: Option<i32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdatePantryItem {
    pub food: String,
    pub available: bool,
    #[serde(default)]
    pub amount: Option<f64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdatePantryParams {
    pub items: Vec<UpdatePantryItem>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetMealPlansParams {
    pub from_date: String, // YYYY-MM-DD format
    pub to_date: String,   // YYYY-MM-DD format
    #[serde(default)]
    pub meal_type: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateMealPlanParams {
    #[serde(default)]
    pub recipe_id: Option<i32>,
    #[serde(default)]
    pub title: Option<String>,
    pub servings: i32,
    pub date: String, // YYYY-MM-DD format
    pub meal_type: i32,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DeleteMealPlanParams {
    pub id: i32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetCookLogParams {
    #[serde(default)]
    pub recipe_id: Option<i32>,
    #[serde(default = "default_days_back")]
    pub days_back: i32,
}

fn default_days_back() -> i32 {
    30
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct LogCookedRecipeParams {
    pub recipe_id: i32,
    #[serde(default = "default_servings")]
    pub servings: i32,
    #[serde(default)]
    pub rating: Option<i32>,
    #[serde(default)]
    pub comment: Option<String>,
}

fn default_servings() -> i32 {
    1
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SuggestFromInventoryParams {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_days_until_expiry")]
    pub days_until_expiry: i32,
}

fn default_mode() -> String {
    "maximum-use".to_string()
}

fn default_days_until_expiry() -> i32 {
    3
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateRecipeKeywordsParams {
    /// The recipe ID to update
    pub recipe_id: i32,
    /// List of keyword names to set or append
    pub keywords: Vec<String>,
    /// "set" (default) replaces all keywords; "add" appends to existing
    #[serde(default = "default_keyword_mode")]
    pub mode: String,
}

fn default_keyword_mode() -> String {
    "set".to_string()
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateRecipeBookParams {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AddRecipeToBookParams {
    pub book_id: i32,
    pub recipe_id: i32,
}

// Global shared authentication state
/// Global authentication token storage to handle Tandoor's rate limiting
static GLOBAL_AUTH: OnceLock<Arc<Mutex<Option<String>>>> = OnceLock::new();

/// Global credentials storage for automatic re-authentication
static GLOBAL_CREDENTIALS: OnceLock<(String, String)> = OnceLock::new();

/// # Tandoor MCP Server
///
/// The main MCP server implementation that provides Tandoor functionality through
/// standardized tools. This server handles all communication with Tandoor APIs
/// and exposes them as MCP tools for AI assistants.
///
/// ## Rate Limiting Considerations
///
/// Tandoor has aggressive rate limiting on authentication endpoints (10 requests/day).
/// The server handles this by:
/// - Storing authentication tokens globally
/// - Reusing tokens across requests
/// - Supporting pre-set tokens via environment variables
///
/// ## Example Usage
///
/// ```no_run
/// use mcp_tandoor::server::TandoorMcpServer;
///
/// // Create server with credentials
/// let server = TandoorMcpServer::new_with_credentials(
///     "http://localhost:8080".to_string(),
///     "admin".to_string(),
///     "password".to_string()
/// );
///
/// // Or set a pre-authenticated token
/// server.set_global_auth_token("your_token".to_string()).await.unwrap();
/// ```
#[derive(Clone)]
pub struct TandoorMcpServer {
    /// Thread-safe client for Tandoor API communication
    client: Arc<Mutex<TandoorClient>>,
    /// MCP tool router for handling tool requests
    tool_router: ToolRouter<TandoorMcpServer>,
}

#[tool_router]
impl TandoorMcpServer {
    /// Create a new Tandoor MCP server with just a base URL.
    /// Authentication will need to be handled separately.
    pub fn new(base_url: String) -> Self {
        Self {
            client: Arc::new(Mutex::new(TandoorClient::new(base_url))),
            tool_router: Self::tool_router(),
        }
    }

    /// Create a new Tandoor MCP server with credentials for automatic authentication.
    ///
    /// The credentials are stored globally and will be used for automatic re-authentication
    /// when tokens expire. This is the recommended constructor for most use cases.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL of the Tandoor server (e.g., "http://localhost:8080")
    /// * `username` - Tandoor username for authentication
    /// * `password` - Tandoor password for authentication
    pub fn new_with_credentials(base_url: String, username: String, password: String) -> Self {
        // Store credentials globally so all instances can use them
        let _ = GLOBAL_CREDENTIALS.set((username, password));

        Self {
            client: Arc::new(Mutex::new(TandoorClient::new(base_url))),
            tool_router: Self::tool_router(),
        }
    }

    /// Set a pre-authenticated token to avoid rate limiting.
    ///
    /// This is useful when you have a token from a previous authentication
    /// or from an external source. Tandoor limits authentication to 10 requests
    /// per day, so reusing tokens is important for reliability.
    ///
    /// # Arguments
    ///
    /// * `token` - A valid Tandoor OAuth2 access token
    pub async fn set_global_auth_token(&self, token: String) -> Result<(), anyhow::Error> {
        let auth_storage = GLOBAL_AUTH.get_or_init(|| Arc::new(Mutex::new(None)));
        let mut auth = auth_storage.lock().await;
        *auth = Some(token);
        tracing::debug!("Global auth token updated");
        Ok(())
    }

    pub async fn authenticate(
        &self,
        username: String,
        password: String,
    ) -> Result<(), anyhow::Error> {
        let mut client = self.client.lock().await;
        let result = client.authenticate(username, password).await;

        if result.is_ok() {
            // Store the token globally for all instances to use
            if let Some(token) = client.get_token() {
                self.set_global_auth_token(token.to_string()).await?;
            }
        }

        result
    }

    async fn ensure_authenticated(
        &self,
    ) -> Result<tokio::sync::MutexGuard<'_, TandoorClient>, anyhow::Error> {
        tracing::info!("=== ensure_authenticated: acquiring client lock ===");

        // Add timeout to client lock acquisition
        let client_result =
            tokio::time::timeout(std::time::Duration::from_secs(5), self.client.lock()).await;

        let mut client = match client_result {
            Ok(client) => {
                tracing::info!("=== ensure_authenticated: client lock acquired ===");
                client
            }
            Err(_) => {
                tracing::error!("=== ensure_authenticated: TIMEOUT acquiring client lock ===");
                return Err(anyhow::anyhow!("Timeout acquiring client lock"));
            }
        };

        if client.is_authenticated() {
            tracing::info!("=== ensure_authenticated: already authenticated ===");
            return Ok(client);
        }

        tracing::info!("=== ensure_authenticated: not authenticated, proceeding with auth ===");

        // If no global token, try to authenticate with stored credentials directly
        if let Some((username, password)) = GLOBAL_CREDENTIALS.get() {
            tracing::info!(
                "Auto-authenticating with stored credentials for user: {}",
                username
            );
            let result = client
                .authenticate(username.clone(), password.clone())
                .await;

            match result {
                Ok(_) => {
                    tracing::info!("Authentication successful");
                    Ok(client)
                }
                Err(e) => {
                    tracing::error!("Authentication failed: {}", e);
                    Err(e)
                }
            }
        } else {
            tracing::error!("No authentication credentials available");
            Err(anyhow::anyhow!("No authentication credentials available"))
        }
    }

    pub async fn test_api_access(&self) -> Result<(), anyhow::Error> {
        // Just check if we can authenticate - don't make actual API calls during startup
        // to avoid holding locks during network I/O
        let client = self.ensure_authenticated().await?;

        // Check if we have a token
        if client.is_authenticated() {
            if let Some(preview) = client.get_token_preview() {
                tracing::info!("Authentication test successful - have token: {}", preview);
            }
            Ok(())
        } else {
            tracing::error!("Authentication test failed - no token available");
            Err(anyhow::anyhow!("No authentication token available"))
        }
        // Lock is automatically released here when client goes out of scope
    }

    // Recipe tools
    #[tool(description = "Search for recipes with flexible querying")]
    async fn search_recipes(
        &self,
        Parameters(params): Parameters<SearchRecipesParams>,
    ) -> Result<CallToolResult, McpError> {
        // Ensure we're authenticated before making API calls
        let client = match self.ensure_authenticated().await {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Authentication failed in search_recipes: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        match client
            .search_recipes(params.query.as_deref(), params.limit, params.page)
            .await
        {
            Ok(response) => {
                let has_next = response.next.is_some();
                let current_page = params.page.unwrap_or(1);
                let recipes_json: Vec<serde_json::Value> = response.results
                    .into_iter()
                    .map(|recipe| {
                        json!({
                            "id": recipe.id,
                            "name": recipe.name,
                            "description": recipe.description,
                            "total_time": recipe.working_time.unwrap_or(0) + recipe.waiting_time.unwrap_or(0),
                            "servings": recipe.servings,
                            "keywords": recipe.keywords.into_iter().map(|k| k.name).collect::<Vec<String>>(),
                            "created": recipe.created,
                            "updated": recipe.updated
                        })
                    })
                    .collect();

                let result = json!({
                    "recipes": recipes_json,
                    "total_count": response.count,
                    "current_page": current_page,
                    "next_page": if has_next { Some(current_page + 1) } else { None },
                    "search_interpretation": format!("Found {} recipes{}",
                        response.count,
                        params.query.as_ref().map_or(String::new(), |q| format!(" matching '{q}'"))
                    )
                });

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                let error = json!({
                    "error": "Failed to search recipes",
                    "details": e.to_string()
                });
                Ok(CallToolResult::error(vec![Content::text(
                    error.to_string(),
                )]))
            }
        }
    }

    #[tool(description = "Get comprehensive recipe information including scaled ingredients")]
    async fn get_recipe_details(
        &self,
        Parameters(params): Parameters<GetRecipeDetailsParams>,
    ) -> Result<CallToolResult, McpError> {
        // Ensure we're authenticated before making API calls
        let client = match self.ensure_authenticated().await {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Authentication failed in get_recipe_details: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        match client.get_recipe(params.id).await {
            Ok(recipe) => {
                let mut ingredients = Vec::new();
                let scaling_factor = if let Some(target_servings) = params.servings {
                    if let Some(original_servings) = recipe.servings {
                        target_servings as f64 / original_servings as f64
                    } else {
                        1.0
                    }
                } else {
                    1.0
                };

                for step in &recipe.steps {
                    for ingredient in &step.ingredients {
                        ingredients.push(json!({
                            "food": ingredient.food.name,
                            "amount": ingredient.amount * scaling_factor,
                            "unit": ingredient.unit.as_ref().map(|u| &u.name),
                            "note": ingredient.note,
                            "is_header": ingredient.is_header,
                            "no_amount": ingredient.no_amount
                        }));
                    }
                }

                let instructions: Vec<String> = recipe
                    .steps
                    .into_iter()
                    .map(|step| {
                        if step.name.is_empty() {
                            step.instruction
                        } else {
                            format!("{}: {}", step.name, step.instruction)
                        }
                    })
                    .collect();

                let result = json!({
                    "id": recipe.id,
                    "name": recipe.name,
                    "description": recipe.description,
                    "instructions": instructions,
                    "ingredients": ingredients,
                    "servings": params.servings.unwrap_or(recipe.servings.unwrap_or(1)),
                    "working_time": recipe.working_time,
                    "waiting_time": recipe.waiting_time,
                    "total_time": recipe.working_time.unwrap_or(0) + recipe.waiting_time.unwrap_or(0),
                    "keywords": recipe.keywords.into_iter().map(|k| k.name).collect::<Vec<String>>(),
                    "nutrition": recipe.nutrition,
                    "created": recipe.created,
                    "updated": recipe.updated,
                    "scaling_applied": scaling_factor != 1.0
                });

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                let error = json!({
                    "error": "Failed to get recipe details",
                    "details": e.to_string()
                });
                Ok(CallToolResult::error(vec![Content::text(
                    error.to_string(),
                )]))
            }
        }
    }

    #[tool(description = "Create a new recipe")]
    async fn create_recipe(
        &self,
        Parameters(params): Parameters<CreateRecipeParams>,
    ) -> Result<CallToolResult, McpError> {
        // Ensure we're authenticated before making API calls
        let client = match self.ensure_authenticated().await {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Authentication failed in create_recipe: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        // Convert keywords from strings to CreateKeywordRequest
        let keywords = params
            .keywords
            .unwrap_or_default()
            .into_iter()
            .map(|name| crate::client::types::CreateKeywordRequest { name })
            .collect();

        // Create a basic step from instructions if provided
        let steps = if let Some(instructions) = params.instructions {
            vec![crate::client::types::CreateStepRequest {
                name: None,
                instruction: instructions,
                ingredients: vec![], // Empty ingredients for now
                time: None,
                order: 1,
            }]
        } else {
            vec![] // Empty steps array if no instructions
        };

        let request = crate::client::types::CreateRecipeRequest {
            name: params.name,
            description: params.description,
            servings: params.servings,
            working_time: params.prep_time.unwrap_or(0),
            waiting_time: params.cook_time.unwrap_or(0),
            keywords,
            steps,
        };

        match client.create_recipe(request).await {
            Ok(recipe) => {
                let result = json!({
                    "id": recipe.id,
                    "name": recipe.name,
                    "description": recipe.description,
                    "servings": recipe.servings,
                    "working_time": recipe.working_time,
                    "waiting_time": recipe.waiting_time,
                    "created": recipe.created,
                    "success": true,
                    "message": "Recipe created successfully"
                });

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                tracing::error!("create_recipe tool failed: {}", e);
                let error = json!({
                    "error": "Failed to create recipe",
                    "details": e.to_string(),
                    "success": false
                });
                Ok(CallToolResult::error(vec![Content::text(
                    error.to_string(),
                )]))
            }
        }
    }

    #[tool(
        description = "Update the keywords/tags on an existing recipe. Use mode='set' to replace all tags (default), or mode='add' to append to existing tags."
    )]
    async fn update_recipe_keywords(
        &self,
        Parameters(params): Parameters<UpdateRecipeKeywordsParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = match self.ensure_authenticated().await {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Authentication failed in update_recipe_keywords: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        // If "add" mode, fetch existing keywords first and merge
        let final_keywords = if params.mode == "add" {
            match client.get_recipe(params.recipe_id).await {
                Ok(recipe) => {
                    let mut all_keywords: Vec<String> =
                        recipe.keywords.into_iter().map(|k| k.name).collect();
                    for kw in &params.keywords {
                        if !all_keywords.contains(kw) {
                            all_keywords.push(kw.clone());
                        }
                    }
                    all_keywords
                }
                Err(e) => {
                    let error = json!({
                        "error": "Failed to fetch existing recipe keywords",
                        "details": e.to_string()
                    });
                    return Ok(CallToolResult::error(vec![Content::text(
                        error.to_string(),
                    )]));
                }
            }
        } else {
            params.keywords
        };

        let keyword_requests = final_keywords
            .into_iter()
            .map(|name| crate::client::types::CreateKeywordRequest { name })
            .collect();

        match client
            .patch_recipe_keywords(params.recipe_id, keyword_requests)
            .await
        {
            Ok(recipe) => {
                let result = json!({
                    "id": recipe.id,
                    "name": recipe.name,
                    "keywords": recipe.keywords.into_iter().map(|k| k.name).collect::<Vec<_>>(),
                    "success": true
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                let error = json!({
                    "error": "Failed to update recipe keywords",
                    "details": e.to_string(),
                    "success": false
                });
                Ok(CallToolResult::error(vec![Content::text(
                    error.to_string(),
                )]))
            }
        }
    }

    #[tool(description = "Get all recipe books")]
    async fn get_recipe_books(&self) -> Result<CallToolResult, McpError> {
        let client = match self.ensure_authenticated().await {
            Ok(c) => c,
            Err(e) => {
                return Ok(CallToolResult::error(vec![Content::text(
                    json!({"error": "Authentication Error", "details": e.to_string()}).to_string(),
                )]));
            }
        };

        match client.get_recipe_books().await {
            Ok(response) => {
                let books: Vec<serde_json::Value> = response
                    .results
                    .into_iter()
                    .map(|b| json!({"id": b.id, "name": b.name, "description": b.description}))
                    .collect();
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(
                        &json!({"books": books, "total_count": response.count}),
                    )
                    .unwrap(),
                )]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(
                json!({"error": "Failed to get recipe books", "details": e.to_string()})
                    .to_string(),
            )])),
        }
    }

    #[tool(description = "Create a new recipe book (collection)")]
    async fn create_recipe_book(
        &self,
        Parameters(params): Parameters<CreateRecipeBookParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = match self.ensure_authenticated().await {
            Ok(c) => c,
            Err(e) => {
                return Ok(CallToolResult::error(vec![Content::text(
                    json!({"error": "Authentication Error", "details": e.to_string()}).to_string(),
                )]));
            }
        };

        let request = crate::client::types::CreateRecipeBookRequest {
            name: params.name,
            description: params.description.unwrap_or_default(),
            shared: vec![],
        };

        match client.create_recipe_book(request).await {
            Ok(book) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(
                    &json!({"id": book.id, "name": book.name, "description": book.description, "success": true}),
                ).unwrap(),
            )])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(
                json!({"error": "Failed to create recipe book", "details": e.to_string()}).to_string(),
            )])),
        }
    }

    #[tool(description = "Add a recipe to a recipe book")]
    async fn add_recipe_to_book(
        &self,
        Parameters(params): Parameters<AddRecipeToBookParams>,
    ) -> Result<CallToolResult, McpError> {
        let client = match self.ensure_authenticated().await {
            Ok(c) => c,
            Err(e) => {
                return Ok(CallToolResult::error(vec![Content::text(
                    json!({"error": "Authentication Error", "details": e.to_string()}).to_string(),
                )]));
            }
        };

        match client.add_recipe_to_book(params.book_id, params.recipe_id).await {
            Ok(entry) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(
                    &json!({"id": entry.id, "book_id": entry.book, "recipe_id": entry.recipe, "success": true}),
                ).unwrap(),
            )])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(
                json!({"error": "Failed to add recipe to book", "details": e.to_string()}).to_string(),
            )])),
        }
    }

    // Shopping list tools
    #[tool(description = "Add items to shopping list with intelligent consolidation")]
    async fn add_to_shopping_list(
        &self,
        Parameters(params): Parameters<AddToShoppingListParams>,
    ) -> Result<CallToolResult, McpError> {
        // Ensure we're authenticated before making API calls
        let client = match self.ensure_authenticated().await {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Authentication failed in add_to_shopping_list: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        if let Some(items) = params.items {
            let mut requests = Vec::new();
            let mut added = Vec::new();
            let mut errors = Vec::new();

            for item in items {
                match client.search_foods(&item.name, Some(1)).await {
                    Ok(foods_response) => {
                        if let Some(food) = foods_response.results.first() {
                            let request = crate::client::types::CreateShoppingListEntryRequest {
                                food: food.id,
                                unit: None,
                                amount: item.amount,
                            };
                            requests.push(request);
                        } else {
                            errors.push(json!({
                                "food": item.name,
                                "error": "Food not found",
                                "suggestion": "Try creating the food first or use a different name"
                            }));
                        }
                    }
                    Err(e) => {
                        errors.push(json!({
                            "food": item.name,
                            "error": "Failed to search for food",
                            "details": e.to_string()
                        }));
                    }
                }
            }

            if !requests.is_empty() {
                match client.add_bulk_to_shopping_list(requests).await {
                    Ok(entries) => {
                        for entry in entries {
                            added.push(json!({
                                "id": entry.id,
                                "food": entry.food.name,
                                "amount": entry.amount,
                                "unit": entry.unit.as_ref().map(|u| &u.name),
                                "status": "added"
                            }));
                        }
                    }
                    Err(e) => {
                        errors.push(json!({
                            "error": "Failed to add items to shopping list",
                            "details": e.to_string()
                        }));
                    }
                }
            }

            let result = json!({
                "added": added,
                "errors": errors,
                "summary": format!("Added {} items, {} errors", added.len(), errors.len())
            });

            Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&result).unwrap(),
            )]))
        } else if let Some(request_text) = params.request {
            let result = json!({
                "message": "Natural language processing not yet implemented",
                "request": request_text,
                "suggestion": "Please use the structured 'items' parameter with an array of {name, amount} objects"
            });

            Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&result).unwrap(),
            )]))
        } else {
            let error = json!({
                "error": "Missing required parameters",
                "message": "Please provide either 'items' array or 'request' text"
            });

            Ok(CallToolResult::error(vec![Content::text(
                error.to_string(),
            )]))
        }
    }

    #[tool(description = "Get current shopping list organized by store section")]
    async fn get_shopping_list(
        &self,
        Parameters(params): Parameters<GetShoppingListParams>,
    ) -> Result<CallToolResult, McpError> {
        // Ensure we're authenticated before making API calls
        let client = match self.ensure_authenticated().await {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Authentication failed in get_shopping_list: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        match client.get_shopping_list().await {
            Ok(response) => {
                let items: Vec<serde_json::Value> = response
                    .results
                    .into_iter()
                    .map(|entry| {
                        json!({
                            "id": entry.id,
                            "food": entry.food.name,
                            "amount": entry.amount,
                            "unit": entry.unit.as_ref().map(|u| &u.name),
                            "checked": entry.checked,
                            "available": entry.food.food_onhand,
                            "created": entry.created,
                            "completed": entry.completed
                        })
                    })
                    .collect();

                let result = if params.format == "grouped" {
                    let mut unchecked = Vec::new();
                    let mut checked = Vec::new();

                    for item in items {
                        if item
                            .get("checked")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            checked.push(item);
                        } else {
                            unchecked.push(item);
                        }
                    }

                    json!({
                        "unchecked_items": unchecked,
                        "checked_items": checked,
                        "total_items": response.count,
                        "format": "grouped"
                    })
                } else {
                    json!({
                        "items": items,
                        "total_items": response.count,
                        "format": "flat"
                    })
                };

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                tracing::error!("get_shopping_list tool failed: {}", e);
                let error = json!({
                    "error": "Failed to get shopping list",
                    "details": e.to_string()
                });
                Ok(CallToolResult::error(vec![Content::text(
                    error.to_string(),
                )]))
            }
        }
    }

    #[tool(description = "Search for foods/ingredients with fuzzy name matching")]
    async fn search_foods(
        &self,
        Parameters(params): Parameters<SearchFoodsParams>,
    ) -> Result<CallToolResult, McpError> {
        // Ensure we're authenticated before making API calls
        let client = match self.ensure_authenticated().await {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Authentication failed in search_foods: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        match client.search_foods(&params.query, params.limit).await {
            Ok(response) => {
                let foods_json: Vec<serde_json::Value> = response
                    .results
                    .into_iter()
                    .map(|food| {
                        json!({
                            "id": food.id,
                            "name": food.name,
                            "plural_name": food.plural_name,
                            "description": food.description,
                            "food_onhand": food.food_onhand,
                            "supermarket_category": food.supermarket_category
                        })
                    })
                    .collect();

                let result = json!({
                    "foods": foods_json,
                    "total_count": response.count,
                    "query": params.query
                });

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                tracing::error!(
                    "search_foods tool failed for query '{}': {}",
                    params.query,
                    e
                );
                let error = json!({
                    "error": "Failed to search foods",
                    "details": e.to_string()
                });
                Ok(CallToolResult::error(vec![Content::text(
                    error.to_string(),
                )]))
            }
        }
    }

    #[tool(description = "Get all available recipe keywords/tags")]
    async fn get_keywords(&self) -> Result<CallToolResult, McpError> {
        tracing::debug!("MCP tool call: get_keywords");

        // Ensure we're authenticated before making API calls
        let client = match self.ensure_authenticated().await {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Authentication failed in get_keywords: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        match client.get_keywords().await {
            Ok(response) => {
                tracing::debug!("Successfully retrieved keywords from Tandoor API");
                let keywords_json: Vec<serde_json::Value> = response
                    .results
                    .into_iter()
                    .map(|keyword| {
                        json!({
                            "id": keyword.id,
                            "name": keyword.name,
                            "description": keyword.description
                        })
                    })
                    .collect();

                let result = json!({
                    "keywords": keywords_json,
                    "total_count": response.count
                });

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                tracing::error!("get_keywords tool failed: {}", e);

                // Provide more specific error information
                let error_details = if e.to_string().contains("Not authenticated") {
                    json!({
                        "error": "Authentication Error",
                        "message": "Your authentication token has expired or is invalid",
                        "details": e.to_string(),
                        "suggestion": "Please restart the MCP server to re-authenticate with Tandoor"
                    })
                } else if e.to_string().contains("Failed to connect") {
                    json!({
                        "error": "Connection Error",
                        "message": "Unable to connect to Tandoor server",
                        "details": e.to_string(),
                        "suggestion": "Check that Tandoor is running and accessible at the configured URL"
                    })
                } else {
                    json!({
                        "error": "Failed to get keywords",
                        "message": "An unexpected error occurred while fetching keywords",
                        "details": e.to_string(),
                        "suggestion": "Check server logs for more details"
                    })
                };

                Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error_details).unwrap(),
                )]))
            }
        }
    }

    #[tool(description = "Get available measurement units")]
    async fn get_units(&self) -> Result<CallToolResult, McpError> {
        tracing::info!("=== MCP tool call: get_units started ===");

        // Ensure we're authenticated before making API calls
        tracing::info!("Checking authentication for get_units");
        let client = match self.ensure_authenticated().await {
            Ok(client) => {
                tracing::info!("Authentication successful, proceeding with get_units API");
                client
            }
            Err(e) => {
                tracing::error!("Authentication failed in get_units: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        match client.get_units().await {
            Ok(response) => {
                tracing::debug!("Successfully retrieved {} units", response.count);
                let units_json: Vec<serde_json::Value> = response
                    .results
                    .into_iter()
                    .map(|unit| {
                        json!({
                            "id": unit.id,
                            "name": unit.name,
                            "plural_name": unit.plural_name,
                            "description": unit.description,
                            "base_unit": unit.base_unit,
                            "type": unit.type_
                        })
                    })
                    .collect();

                let result = json!({
                    "units": units_json,
                    "total_count": response.count
                });

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                tracing::error!("get_units tool failed: {}", e);
                let error = json!({
                    "error": "Failed to get units",
                    "details": e.to_string()
                });
                Ok(CallToolResult::error(vec![Content::text(
                    error.to_string(),
                )]))
            }
        }
    }

    // Meal planning tools
    #[tool(description = "Get meal plans for a date range")]
    async fn get_meal_plans(
        &self,
        Parameters(params): Parameters<GetMealPlansParams>,
    ) -> Result<CallToolResult, McpError> {
        // Ensure we're authenticated before making API calls
        let client = match self.ensure_authenticated().await {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Authentication failed in get_meal_plans: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        match client
            .get_meal_plans(Some(&params.from_date), Some(&params.to_date))
            .await
        {
            Ok(response) => {
                let meal_plans_json: Vec<serde_json::Value> = response
                    .results
                    .into_iter()
                    .filter(|plan| {
                        params.meal_type.as_ref().is_none_or(|mt| {
                            plan.meal_type.name.to_lowercase() == mt.to_lowercase()
                        })
                    })
                    .map(|plan| {
                        json!({
                            "id": plan.id,
                            "date": plan.date,
                            "meal_type": plan.meal_type.name,
                            "recipe_id": plan.recipe.as_ref().map(|r| r.id),
                            "recipe_name": plan.recipe.as_ref().map(|r| &r.name),
                            "title": plan.title,
                            "servings": plan.servings,
                            "note": plan.note,
                            "created": plan.created
                        })
                    })
                    .collect();

                let total_count = meal_plans_json.len();
                let result = json!({
                    "meal_plans": meal_plans_json,
                    "total_count": total_count,
                    "date_range": format!("{} to {}", params.from_date, params.to_date),
                    "meal_type_filter": params.meal_type
                });

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                let error = json!({
                    "error": "Failed to get meal plans",
                    "details": e.to_string()
                });
                Ok(CallToolResult::error(vec![Content::text(
                    error.to_string(),
                )]))
            }
        }
    }

    #[tool(description = "Create a new meal plan")]
    async fn create_meal_plan(
        &self,
        Parameters(params): Parameters<CreateMealPlanParams>,
    ) -> Result<CallToolResult, McpError> {
        // Ensure we're authenticated before making API calls
        let client = match self.ensure_authenticated().await {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Authentication failed in create_meal_plan: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        let date = chrono::NaiveDate::parse_from_str(&params.date, "%Y-%m-%d").map_err(|e| {
            McpError::invalid_params(
                "Invalid date format",
                Some(serde_json::json!({"error": e.to_string()})),
            )
        })?;

        let request = crate::client::types::CreateMealPlanRequest {
            recipe: params.recipe_id,
            title: params.title,
            servings: params.servings,
            date,
            meal_type: params.meal_type,
            note: params.note,
        };

        match client.create_meal_plan(request).await {
            Ok(meal_plan) => {
                let result = json!({
                    "id": meal_plan.id,
                    "date": meal_plan.date,
                    "meal_type": meal_plan.meal_type.name,
                    "recipe_id": meal_plan.recipe.as_ref().map(|r| r.id),
                    "recipe_name": meal_plan.recipe.as_ref().map(|r| &r.name),
                    "title": meal_plan.title,
                    "servings": meal_plan.servings,
                    "note": meal_plan.note,
                    "created": meal_plan.created,
                    "success": true,
                    "message": "Meal plan created successfully"
                });

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                let error = json!({
                    "error": "Failed to create meal plan",
                    "details": e.to_string(),
                    "success": false
                });
                Ok(CallToolResult::error(vec![Content::text(
                    error.to_string(),
                )]))
            }
        }
    }

    #[tool(description = "Delete a meal plan")]
    async fn delete_meal_plan(
        &self,
        Parameters(params): Parameters<DeleteMealPlanParams>,
    ) -> Result<CallToolResult, McpError> {
        // Ensure we're authenticated before making API calls
        let client = match self.ensure_authenticated().await {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Authentication failed in delete_meal_plan: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        match client.delete_meal_plan(params.id).await {
            Ok(_) => {
                let result = json!({
                    "deleted": {
                        "id": params.id
                    },
                    "success": true,
                    "message": "Meal plan deleted successfully"
                });

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                let error = json!({
                    "error": "Failed to delete meal plan",
                    "details": e.to_string(),
                    "success": false
                });
                Ok(CallToolResult::error(vec![Content::text(
                    error.to_string(),
                )]))
            }
        }
    }

    #[tool(description = "Get available meal types")]
    async fn get_meal_types(&self) -> Result<CallToolResult, McpError> {
        // Ensure we're authenticated before making API calls
        let client = match self.ensure_authenticated().await {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Authentication failed in get_meal_types: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        match client.get_meal_types().await {
            Ok(response) => {
                let meal_types_json: Vec<serde_json::Value> = response
                    .results
                    .into_iter()
                    .map(|meal_type| {
                        json!({
                            "id": meal_type.id,
                            "name": meal_type.name,
                            "order": meal_type.order,
                            "icon": meal_type.icon,
                            "color": meal_type.color
                        })
                    })
                    .collect();

                let result = json!({
                    "meal_types": meal_types_json,
                    "total_count": response.count
                });

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                let error = json!({
                    "error": "Failed to get meal types",
                    "details": e.to_string()
                });
                Ok(CallToolResult::error(vec![Content::text(
                    error.to_string(),
                )]))
            }
        }
    }

    // Shopping list management tools
    #[tool(description = "Mark shopping list items as checked/purchased")]
    async fn check_shopping_items(
        &self,
        Parameters(params): Parameters<CheckShoppingItemsParams>,
    ) -> Result<CallToolResult, McpError> {
        // Ensure we're authenticated before making API calls
        let client = match self.ensure_authenticated().await {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Authentication failed in check_shopping_items: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        let mut updated = Vec::new();
        let mut errors = Vec::new();

        for item in params.items {
            if let Some(item_id) = item.as_i64() {
                let request = crate::client::types::UpdateShoppingListEntryRequest {
                    checked: Some(true),
                    amount: None,
                };

                match client
                    .update_shopping_list_entry(item_id as i32, request)
                    .await
                {
                    Ok(entry) => {
                        updated.push(json!({
                            "id": entry.id,
                            "food": entry.food.name,
                            "checked": entry.checked,
                            "status": "checked"
                        }));
                    }
                    Err(e) => {
                        errors.push(json!({
                            "item_id": item_id,
                            "error": "Failed to update item",
                            "details": e.to_string()
                        }));
                    }
                }
            } else if let Some(item_name) = item.as_str() {
                match client.get_shopping_list().await {
                    Ok(list_response) => {
                        if let Some(entry) = list_response.results.iter().find(|e| {
                            e.food
                                .name
                                .to_lowercase()
                                .contains(&item_name.to_lowercase())
                        }) {
                            let request = crate::client::types::UpdateShoppingListEntryRequest {
                                checked: Some(true),
                                amount: None,
                            };

                            match client.update_shopping_list_entry(entry.id, request).await {
                                Ok(updated_entry) => {
                                    updated.push(json!({
                                        "id": updated_entry.id,
                                        "food": updated_entry.food.name,
                                        "checked": updated_entry.checked,
                                        "status": "checked"
                                    }));
                                }
                                Err(e) => {
                                    errors.push(json!({
                                        "item_name": item_name,
                                        "error": "Failed to update item",
                                        "details": e.to_string()
                                    }));
                                }
                            }
                        } else {
                            errors.push(json!({
                                "item_name": item_name,
                                "error": "Item not found in shopping list"
                            }));
                        }
                    }
                    Err(e) => {
                        errors.push(json!({
                            "item_name": item_name,
                            "error": "Failed to get shopping list",
                            "details": e.to_string()
                        }));
                    }
                }
            }
        }

        let result = json!({
            "updated": updated,
            "errors": errors,
            "summary": format!("Checked {} items, {} errors", updated.len(), errors.len())
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap(),
        )]))
    }

    #[tool(description = "Clear checked items from shopping list and update pantry")]
    async fn clear_shopping_list(&self) -> Result<CallToolResult, McpError> {
        // Ensure we're authenticated before making API calls
        let client = match self.ensure_authenticated().await {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Authentication failed in clear_shopping_list: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        match client.get_shopping_list().await {
            Ok(response) => {
                let mut removed_items = Vec::new();
                let mut pantry_updates = Vec::new();
                let mut errors = Vec::new();

                for entry in response.results {
                    if entry.checked {
                        match client.delete_shopping_list_entry(entry.id).await {
                            Ok(_) => {
                                removed_items.push(json!({
                                    "id": entry.id,
                                    "food": entry.food.name,
                                    "amount": entry.amount,
                                    "unit": entry.unit.as_ref().map(|u| &u.name),
                                    "was_checked": entry.checked
                                }));

                                match client.update_food_availability(entry.food.id, true).await {
                                    Ok(_) => {
                                        pantry_updates.push(entry.food.name.clone());
                                    }
                                    Err(e) => {
                                        errors.push(json!({
                                            "food": entry.food.name,
                                            "error": "Failed to update pantry",
                                            "details": e.to_string()
                                        }));
                                    }
                                }
                            }
                            Err(e) => {
                                errors.push(json!({
                                    "food": entry.food.name,
                                    "error": "Failed to remove from shopping list",
                                    "details": e.to_string()
                                }));
                            }
                        }
                    }
                }

                let result = json!({
                    "removed_items": removed_items,
                    "pantry_updates": pantry_updates,
                    "errors": errors,
                    "summary": format!("Removed {} checked items, updated pantry for {} items",
                        removed_items.len(), pantry_updates.len())
                });

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                let error = json!({
                    "error": "Failed to get shopping list",
                    "details": e.to_string()
                });
                Ok(CallToolResult::error(vec![Content::text(
                    error.to_string(),
                )]))
            }
        }
    }

    // Inventory management tools
    #[tool(description = "Update pantry inventory status")]
    async fn update_pantry(
        &self,
        Parameters(params): Parameters<UpdatePantryParams>,
    ) -> Result<CallToolResult, McpError> {
        // Ensure we're authenticated before making API calls
        let client = match self.ensure_authenticated().await {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Authentication failed in update_pantry: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        let mut updated = Vec::new();
        let mut errors = Vec::new();

        for item in params.items {
            match client.search_foods(&item.food, Some(1)).await {
                Ok(foods_response) => {
                    if let Some(food) = foods_response.results.first() {
                        match client
                            .update_food_availability(food.id, item.available)
                            .await
                        {
                            Ok(updated_food) => {
                                updated.push(json!({
                                    "id": updated_food.id,
                                    "name": updated_food.name,
                                    "available": updated_food.food_onhand,
                                    "amount": item.amount,
                                    "status": "updated"
                                }));
                            }
                            Err(e) => {
                                errors.push(json!({
                                    "food": item.food,
                                    "error": "Failed to update availability",
                                    "details": e.to_string()
                                }));
                            }
                        }
                    } else {
                        errors.push(json!({
                            "food": item.food,
                            "error": "Food not found",
                            "suggestion": "Try creating the food first or use a different name"
                        }));
                    }
                }
                Err(e) => {
                    errors.push(json!({
                        "food": item.food,
                        "error": "Failed to search for food",
                        "details": e.to_string()
                    }));
                }
            }
        }

        let result = json!({
            "updated": updated,
            "errors": errors,
            "summary": format!("Updated {} items, {} errors", updated.len(), errors.len())
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap(),
        )]))
    }

    // Recipe history tools
    #[tool(description = "Get cooking history")]
    async fn get_cook_log(
        &self,
        Parameters(params): Parameters<GetCookLogParams>,
    ) -> Result<CallToolResult, McpError> {
        // Ensure we're authenticated before making API calls
        let client = match self.ensure_authenticated().await {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Authentication failed in get_cook_log: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        match client
            .get_cook_log(params.recipe_id, Some(params.days_back))
            .await
        {
            Ok(response) => {
                let cook_log_json: Vec<serde_json::Value> = response
                    .results
                    .into_iter()
                    .map(|log| {
                        json!({
                            "id": log.id,
                            "recipe_id": log.recipe.id,
                            "recipe_name": log.recipe.name,
                            "servings": log.servings,
                            "rating": log.rating,
                            "comment": log.comment,
                            "created": log.created,
                            "date_cooked": log.created.format("%Y-%m-%d").to_string()
                        })
                    })
                    .collect();

                let result = json!({
                    "cook_log": cook_log_json,
                    "total_count": response.count,
                    "days_back": params.days_back,
                    "recipe_filter": params.recipe_id
                });

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                let error = json!({
                    "error": "Failed to get cook log",
                    "details": e.to_string()
                });
                Ok(CallToolResult::error(vec![Content::text(
                    error.to_string(),
                )]))
            }
        }
    }

    #[tool(description = "Log a cooked recipe")]
    async fn log_cooked_recipe(
        &self,
        Parameters(params): Parameters<LogCookedRecipeParams>,
    ) -> Result<CallToolResult, McpError> {
        // Ensure we're authenticated before making API calls
        let client = match self.ensure_authenticated().await {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Authentication failed in log_cooked_recipe: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        let request = crate::client::types::CreateCookLogRequest {
            recipe: params.recipe_id,
            servings: params.servings,
            rating: params.rating,
            comment: params.comment,
        };

        match client.log_cooked_recipe(request).await {
            Ok(cook_log) => {
                let result = json!({
                    "id": cook_log.id,
                    "recipe_id": cook_log.recipe.id,
                    "recipe_name": cook_log.recipe.name,
                    "servings": cook_log.servings,
                    "rating": cook_log.rating,
                    "comment": cook_log.comment,
                    "created": cook_log.created,
                    "success": true,
                    "message": "Recipe cooking logged successfully"
                });

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                let error = json!({
                    "error": "Failed to log cooked recipe",
                    "details": e.to_string(),
                    "success": false
                });
                Ok(CallToolResult::error(vec![Content::text(
                    error.to_string(),
                )]))
            }
        }
    }

    #[tool(description = "Get recipe suggestions based on current inventory")]
    async fn suggest_from_inventory(
        &self,
        Parameters(params): Parameters<SuggestFromInventoryParams>,
    ) -> Result<CallToolResult, McpError> {
        // Ensure we're authenticated before making API calls
        let client = match self.ensure_authenticated().await {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Authentication failed in suggest_from_inventory: {}", e);
                let error = json!({
                    "error": "Authentication Error",
                    "message": "Failed to authenticate with Tandoor",
                    "details": e.to_string(),
                    "suggestion": "Check your Tandoor credentials and server connectivity"
                });
                return Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string_pretty(&error).unwrap(),
                )]));
            }
        };

        // Get available foods in pantry
        match client.search_foods("", Some(100)).await {
            Ok(foods_response) => {
                let available_foods: Vec<&crate::client::types::Food> = foods_response
                    .results
                    .iter()
                    .filter(|food| food.food_onhand)
                    .collect();

                if available_foods.is_empty() {
                    let result = json!({
                        "suggestions": [],
                        "message": "No ingredients found in pantry. Update your inventory first.",
                        "mode": params.mode
                    });
                    return Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string_pretty(&result).unwrap(),
                    )]));
                }

                // Search for recipes that can use these ingredients
                match client.search_recipes(None, Some(20), None).await {
                    Ok(recipes_response) => {
                        let mut recipe_suggestions = Vec::new();

                        for recipe in recipes_response.results {
                            // Get recipe details to check ingredients
                            if let Ok(detailed_recipe) = client.get_recipe(recipe.id).await {
                                let mut matching_ingredients = 0;
                                let mut total_ingredients = 0;
                                let mut missing_ingredients = Vec::new();

                                for step in &detailed_recipe.steps {
                                    for ingredient in &step.ingredients {
                                        if !ingredient.is_header && !ingredient.no_amount {
                                            total_ingredients += 1;

                                            let ingredient_available =
                                                available_foods.iter().any(|food| {
                                                    food.name.to_lowercase()
                                                        == ingredient.food.name.to_lowercase()
                                                });

                                            if ingredient_available {
                                                matching_ingredients += 1;
                                            } else {
                                                missing_ingredients
                                                    .push(ingredient.food.name.clone());
                                            }
                                        }
                                    }
                                }

                                if total_ingredients > 0 {
                                    let match_percentage = (matching_ingredients as f64
                                        / total_ingredients as f64)
                                        * 100.0;

                                    // Filter based on mode
                                    let should_include = match params.mode.as_str() {
                                        "maximum-use" => match_percentage >= 50.0, // At least 50% match
                                        "expiring" => {
                                            match_percentage >= 30.0
                                                && missing_ingredients.len() <= 3
                                        } // Good match with few missing items
                                        _ => match_percentage >= 60.0,
                                    };

                                    if should_include {
                                        let reason = if params.mode == "expiring" {
                                            format!("Uses {:.0}% of pantry ingredients, only {} missing items", match_percentage, missing_ingredients.len())
                                        } else {
                                            format!(
                                                "Uses {match_percentage:.0}% of available ingredients"
                                            )
                                        };

                                        recipe_suggestions.push(json!({
                                            "recipe_id": recipe.id,
                                            "recipe_name": recipe.name,
                                            "match_percentage": match_percentage,
                                            "matching_ingredients": matching_ingredients,
                                            "total_ingredients": total_ingredients,
                                            "missing_ingredients": missing_ingredients,
                                            "reason": reason,
                                            "total_time": recipe.working_time.unwrap_or(0) + recipe.waiting_time.unwrap_or(0)
                                        }));
                                    }
                                }
                            }
                        }

                        // Sort by match percentage
                        recipe_suggestions.sort_by(|a, b| {
                            let a_match = a
                                .get("match_percentage")
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                            let b_match = b
                                .get("match_percentage")
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                            b_match
                                .partial_cmp(&a_match)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });

                        // Take top 10
                        recipe_suggestions.truncate(10);

                        let suggestion_count = recipe_suggestions.len();
                        let result = json!({
                            "suggestions": recipe_suggestions,
                            "available_ingredients": available_foods.iter().map(|f| &f.name).collect::<Vec<_>>(),
                            "mode": params.mode,
                            "total_available": available_foods.len(),
                            "message": format!("Found {} recipe suggestions using your {} available ingredients",
                                suggestion_count, available_foods.len())
                        });

                        Ok(CallToolResult::success(vec![Content::text(
                            serde_json::to_string_pretty(&result).unwrap(),
                        )]))
                    }
                    Err(e) => {
                        let error = json!({
                            "error": "Failed to search recipes",
                            "details": e.to_string()
                        });
                        Ok(CallToolResult::error(vec![Content::text(
                            error.to_string(),
                        )]))
                    }
                }
            }
            Err(e) => {
                let error = json!({
                    "error": "Failed to get inventory",
                    "details": e.to_string()
                });
                Ok(CallToolResult::error(vec![Content::text(
                    error.to_string(),
                )]))
            }
        }
    }
}

#[tool_handler]
impl ServerHandler for TandoorMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("This server provides comprehensive tools for managing recipes, shopping lists, meal plans, and food inventory through the Tandoor recipe management system. Available tools include: recipe search and management, shopping list operations, meal planning, inventory tracking, cooking history, and recipe suggestions based on available ingredients.".to_string()),
        }
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        Ok(self.get_info())
    }
}
