//! HTTP client implementation for the Tandoor API.

use crate::client::{auth::TandoorAuth, types::*};
use anyhow::Result;
use reqwest::Client;

/// # Tandoor HTTP Client
///
/// A comprehensive HTTP client for interacting with the Tandoor recipe management API.
/// Handles authentication, recipes, shopping lists, meal planning, and more.
///
/// ## Features
///
/// - **OAuth2 Authentication**: Handles Tandoor's token-based authentication
/// - **Recipe Management**: Search, create, retrieve, and manage recipes
/// - **Shopping Lists**: Add items, manage lists, check off completed items
/// - **Meal Planning**: Plan meals and add ingredients to shopping lists
/// - **Food Search**: Find foods and ingredients in the database
/// - **Import Support**: Import recipes from URLs
/// - **Rate Limit Handling**: Works with Tandoor's authentication limits
///
/// ## Example
///
/// ```no_run
/// use mcp_tandoor::client::TandoorClient;
///
/// # async fn example() -> anyhow::Result<()> {
/// let mut client = TandoorClient::new("http://localhost:8080".to_string());
///
/// // Authenticate
/// client.authenticate("username".to_string(), "password".to_string()).await?;
///
/// // Search recipes
/// let recipes = client.search_recipes(Some("pasta"), Some(10)).await?;
///
/// // Get recipe details  
/// if let Some(recipe) = recipes.results.first() {
///     let details = client.get_recipe(recipe.id).await?;
///     println!("Recipe: {}", details.name);
/// }
/// # Ok(())
/// # }
/// ```
pub struct TandoorClient {
    /// Base URL of the Tandoor server
    base_url: String,
    /// HTTP client for making requests
    client: Client,
    /// Authentication handler
    auth: TandoorAuth,
}

impl TandoorClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: Client::new(),
            auth: TandoorAuth::new(base_url.clone()),
            base_url,
        }
    }

    pub async fn authenticate(&mut self, username: String, password: String) -> Result<()> {
        self.auth.authenticate(username, password).await
    }

    pub fn is_authenticated(&self) -> bool {
        self.auth.is_authenticated()
    }

    pub fn get_token_preview(&self) -> Option<String> {
        self.auth
            .get_token()
            .map(|t| format!("{}...", &t[..t.len().min(10)]))
    }

    pub fn set_token(&mut self, token: String) {
        self.auth.set_token(token);
    }

    pub fn get_token(&self) -> Option<&str> {
        self.auth.get_token()
    }

    fn get_auth_header(&self) -> Result<String> {
        match self.auth.get_token() {
            Some(token) => {
                tracing::trace!(
                    "Using authentication token: {}...",
                    &token[..token.len().min(10)]
                );
                // Tandoor uses OAuth2 access tokens with Bearer authentication
                Ok(format!("Bearer {token}"))
            }
            None => {
                tracing::warn!("Attempted to make authenticated request without valid token - server will handle re-authentication if credentials are available");
                anyhow::bail!("Not authenticated - please verify credentials and re-authenticate")
            }
        }
    }

    // Recipe operations
    pub async fn search_recipes(
        &self,
        query: Option<&str>,
        limit: Option<i32>,
        page: Option<i32>,
    ) -> Result<PaginatedResponse<Recipe>> {
        let auth_header = self.get_auth_header()?;
        let mut url = format!("{}/api/recipe/", self.base_url);

        let mut params = vec![];
        if let Some(q) = query {
            params.push(format!("query={}", urlencoding::encode(q)));
        }
        if let Some(l) = limit {
            params.push(format!("page_size={l}"));
        }
        if let Some(p) = page {
            params.push(format!("page={p}"));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        tracing::debug!("Searching recipes with URL: {}", url);
        tracing::trace!("Search parameters - query: {:?}, limit: {:?}", query, limit);

        let response = self
            .client
            .get(&url)
            .header("Authorization", auth_header)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Network error searching recipes: {}", e);
                anyhow::anyhow!("Failed to connect to Tandoor API: {e}")
            })?;

        let status = response.status();
        tracing::trace!("Recipe search response status: {}", status);

        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error response".to_string());
            tracing::error!(
                "Failed to search recipes with status {}: {}",
                status,
                error_body
            );
            anyhow::bail!("Failed to search recipes: {status} - {error_body}");
        }

        let recipes = response.json().await.map_err(|e| {
            tracing::error!("Failed to parse recipe search response: {}", e);
            anyhow::anyhow!("Invalid response format: {e}")
        })?;

        tracing::debug!("Recipe search successful");
        Ok(recipes)
    }

    pub async fn get_recipe(&self, id: i32) -> Result<Recipe> {
        let auth_header = self.get_auth_header()?;
        let url = format!("{}/api/recipe/{}/", self.base_url, id);

        tracing::debug!("Getting recipe details for ID: {}", id);

        let response = self
            .client
            .get(&url)
            .header("Authorization", auth_header)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Network error getting recipe {}: {}", id, e);
                anyhow::anyhow!("Failed to connect to Tandoor API: {e}")
            })?;

        let status = response.status();
        tracing::trace!("Get recipe response status: {}", status);

        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error response".to_string());
            tracing::error!(
                "Failed to get recipe {} with status {}: {}",
                id,
                status,
                error_body
            );

            match status.as_u16() {
                404 => anyhow::bail!("Recipe with ID {id} not found"),
                403 => anyhow::bail!("Access denied to recipe {id}"),
                _ => anyhow::bail!("Failed to get recipe: {status} - {error_body}"),
            }
        }

        let recipe = response.json().await.map_err(|e| {
            tracing::error!("Failed to parse recipe response: {}", e);
            anyhow::anyhow!("Invalid response format: {e}")
        })?;

        tracing::debug!("Successfully retrieved recipe: {}", id);
        Ok(recipe)
    }

    pub async fn create_recipe(&self, request: CreateRecipeRequest) -> Result<Recipe> {
        let auth_header = self.get_auth_header()?;
        let url = format!("{}/api/recipe/", self.base_url);

        tracing::debug!("Creating new recipe: {}", request.name);
        tracing::trace!(
            "Recipe details: servings={:?}, working_time={:?}, waiting_time={:?}",
            request.servings,
            request.working_time,
            request.waiting_time
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", auth_header)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Network error creating recipe: {}", e);
                anyhow::anyhow!("Failed to connect to Tandoor API: {e}")
            })?;

        let status = response.status();
        tracing::trace!("Create recipe response status: {}", status);

        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error response".to_string());
            tracing::error!(
                "Failed to create recipe '{}' with status {}: {}",
                request.name,
                status,
                error_body
            );
            anyhow::bail!("Failed to create recipe: {status} - {error_body}");
        }

        let recipe: Recipe = response.json().await.map_err(|e| {
            tracing::error!("Failed to parse create recipe response: {}", e);
            anyhow::anyhow!("Invalid response format: {e}")
        })?;

        tracing::info!(
            "Successfully created recipe '{}' with ID: {}",
            request.name,
            recipe.id
        );
        Ok(recipe)
    }

    pub async fn patch_recipe_keywords(
        &self,
        recipe_id: i32,
        keywords: Vec<CreateKeywordRequest>,
    ) -> Result<Recipe> {
        let auth_header = self.get_auth_header()?;
        let url = format!("{}/api/recipe/{}/", self.base_url, recipe_id);
        let request = UpdateRecipeKeywordsRequest { keywords };

        let response = self
            .client
            .patch(&url)
            .header("Authorization", auth_header)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to update recipe keywords: {body}");
        }

        let recipe = response.json().await?;
        Ok(recipe)
    }

    pub async fn import_recipe_from_url(&self, url: &str) -> Result<Recipe> {
        let auth_header = self.get_auth_header()?;
        let import_url = format!("{}/api/recipe-from-source/", self.base_url);
        let request = RecipeImport {
            url: url.to_string(),
        };

        tracing::info!("Importing recipe from URL: {}", url);

        let response = self
            .client
            .post(&import_url)
            .header("Authorization", auth_header)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Network error importing recipe from {}: {}", url, e);
                anyhow::anyhow!("Failed to connect to Tandoor API: {e}")
            })?;

        let status = response.status();
        tracing::trace!("Import recipe response status: {}", status);

        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error response".to_string());
            tracing::error!(
                "Failed to import recipe from {} with status {}: {}",
                url,
                status,
                error_body
            );

            match status.as_u16() {
                400 => anyhow::bail!("Invalid URL or unsupported recipe site: {url}"),
                404 => anyhow::bail!("Recipe import endpoint not available"),
                _ => anyhow::bail!("Failed to import recipe: {status} - {error_body}"),
            }
        }

        let recipe: Recipe = response.json().await.map_err(|e| {
            tracing::error!("Failed to parse import recipe response: {}", e);
            anyhow::anyhow!("Invalid response format: {e}")
        })?;

        tracing::info!(
            "Successfully imported recipe '{}' with ID: {}",
            recipe.name,
            recipe.id
        );
        Ok(recipe)
    }

    // Food operations
    pub async fn search_foods(
        &self,
        query: &str,
        limit: Option<i32>,
    ) -> Result<PaginatedResponse<Food>> {
        let auth_header = self.get_auth_header()?;
        let mut url = format!(
            "{}/api/food/?query={}",
            self.base_url,
            urlencoding::encode(query)
        );

        if let Some(l) = limit {
            url.push_str(&format!("&page_size={l}"));
        }

        tracing::debug!(
            "Searching foods with query: '{}', limit: {:?}",
            query,
            limit
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", auth_header)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Network error searching foods: {}", e);
                anyhow::anyhow!("Failed to connect to Tandoor API: {e}")
            })?;

        let status = response.status();
        tracing::trace!("Food search response status: {}", status);

        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error response".to_string());
            tracing::error!(
                "Failed to search foods '{}' with status {}: {}",
                query,
                status,
                error_body
            );
            anyhow::bail!("Failed to search foods: {status} - {error_body}");
        }

        let foods: PaginatedResponse<Food> = response.json().await.map_err(|e| {
            tracing::error!("Failed to parse food search response: {}", e);
            anyhow::anyhow!("Invalid response format: {e}")
        })?;

        tracing::debug!("Food search successful, found {} results", foods.count);
        Ok(foods)
    }

    pub async fn update_food_availability(&self, food_id: i32, available: bool) -> Result<Food> {
        let auth_header = self.get_auth_header()?;
        let url = format!("{}/api/food/{}/", self.base_url, food_id);
        let request = UpdateFoodRequest {
            food_onhand: Some(available),
        };

        let response = self
            .client
            .patch(&url)
            .header("Authorization", auth_header)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to update food availability: {}", response.status());
        }

        let food = response.json().await?;
        Ok(food)
    }

    // Shopping list operations
    pub async fn get_shopping_list(&self) -> Result<PaginatedResponse<ShoppingListEntry>> {
        let auth_header = self.get_auth_header()?;
        let url = format!("{}/api/shopping-list-entry/", self.base_url);

        tracing::debug!("Fetching shopping list");

        let response = self
            .client
            .get(&url)
            .header("Authorization", auth_header)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Network error getting shopping list: {}", e);
                anyhow::anyhow!("Failed to connect to Tandoor API: {e}")
            })?;

        let status = response.status();
        tracing::trace!("Shopping list response status: {}", status);

        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error response".to_string());
            tracing::error!(
                "Failed to get shopping list with status {}: {}",
                status,
                error_body
            );
            anyhow::bail!("Failed to get shopping list: {status} - {error_body}");
        }

        // Handle both paginated response and simple array response
        let response_text = response.text().await.map_err(|e| {
            tracing::error!("Failed to read shopping list response: {}", e);
            anyhow::anyhow!("Failed to read response: {e}")
        })?;

        let shopping_list: PaginatedResponse<ShoppingListEntry> =
            if response_text.trim().starts_with('[') {
                // Simple array response (empty list case)
                let entries: Vec<ShoppingListEntry> = serde_json::from_str(&response_text)
                    .map_err(|e| {
                        tracing::error!("Failed to parse shopping list array response: {}", e);
                        anyhow::anyhow!("Invalid array response format: {e}")
                    })?;
                PaginatedResponse {
                    count: entries.len() as i32,
                    next: None,
                    previous: None,
                    results: entries,
                }
            } else {
                // Paginated response
                serde_json::from_str(&response_text).map_err(|e| {
                    tracing::error!("Failed to parse shopping list paginated response: {}", e);
                    anyhow::anyhow!("Invalid paginated response format: {e}")
                })?
            };

        tracing::debug!(
            "Successfully retrieved shopping list with {} items",
            shopping_list.count
        );
        Ok(shopping_list)
    }

    pub async fn add_to_shopping_list(
        &self,
        request: CreateShoppingListEntryRequest,
    ) -> Result<ShoppingListEntry> {
        let auth_header = self.get_auth_header()?;
        let url = format!("{}/api/shopping-list-entry/", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", auth_header)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to add to shopping list: {}", response.status());
        }

        let entry = response.json().await?;
        Ok(entry)
    }

    pub async fn add_bulk_to_shopping_list(
        &self,
        entries: Vec<CreateShoppingListEntryRequest>,
    ) -> Result<Vec<ShoppingListEntry>> {
        let auth_header = self.get_auth_header()?;
        let url = format!("{}/api/shopping-list-entry/bulk/", self.base_url);
        let entry_count = entries.len();
        tracing::debug!("Adding {} items to shopping list in bulk", entry_count);
        tracing::trace!("Bulk shopping list items: {:?}", entries);

        let request = BulkShoppingListRequest { entries };

        let response = self
            .client
            .post(&url)
            .header("Authorization", auth_header)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Network error adding bulk items to shopping list: {}", e);
                anyhow::anyhow!("Failed to connect to Tandoor API: {e}")
            })?;

        let status = response.status();
        tracing::trace!("Bulk add response status: {}", status);

        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error response".to_string());
            tracing::error!(
                "Failed to add bulk items with status {}: {}",
                status,
                error_body
            );
            anyhow::bail!("Failed to add bulk to shopping list: {status} - {error_body}");
        }

        let entries: Vec<ShoppingListEntry> = response.json().await.map_err(|e| {
            tracing::error!("Failed to parse bulk add response: {}", e);
            anyhow::anyhow!("Invalid response format: {e}")
        })?;

        tracing::info!(
            "Successfully added {} items to shopping list",
            entries.len()
        );
        Ok(entries)
    }

    pub async fn update_shopping_list_entry(
        &self,
        entry_id: i32,
        request: UpdateShoppingListEntryRequest,
    ) -> Result<ShoppingListEntry> {
        let auth_header = self.get_auth_header()?;
        let url = format!("{}/api/shopping-list-entry/{}/", self.base_url, entry_id);

        let response = self
            .client
            .patch(&url)
            .header("Authorization", auth_header)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Failed to update shopping list entry: {}",
                response.status()
            );
        }

        let entry = response.json().await?;
        Ok(entry)
    }

    pub async fn delete_shopping_list_entry(&self, entry_id: i32) -> Result<()> {
        let auth_header = self.get_auth_header()?;
        let url = format!("{}/api/shopping-list-entry/{}/", self.base_url, entry_id);

        let response = self
            .client
            .delete(&url)
            .header("Authorization", auth_header)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Failed to delete shopping list entry: {}",
                response.status()
            );
        }

        Ok(())
    }

    // Meal planning operations
    pub async fn get_meal_plans(
        &self,
        from_date: Option<&str>,
        to_date: Option<&str>,
    ) -> Result<PaginatedResponse<MealPlan>> {
        let auth_header = self.get_auth_header()?;
        let mut url = format!("{}/api/meal-plan/", self.base_url);

        let mut params = vec![];
        if let Some(from) = from_date {
            params.push(format!("from_date={from}"));
        }
        if let Some(to) = to_date {
            params.push(format!("to_date={to}"));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        tracing::debug!("Getting meal plans from {:?} to {:?}", from_date, to_date);

        let response = self
            .client
            .get(&url)
            .header("Authorization", auth_header)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Network error getting meal plans: {}", e);
                anyhow::anyhow!("Failed to connect to Tandoor API: {e}")
            })?;

        let status = response.status();
        tracing::trace!("Meal plans response status: {}", status);

        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error response".to_string());
            tracing::error!(
                "Failed to get meal plans with status {}: {}",
                status,
                error_body
            );
            anyhow::bail!("Failed to get meal plans: {status} - {error_body}");
        }

        let meal_plans: PaginatedResponse<MealPlan> = response.json().await.map_err(|e| {
            tracing::error!("Failed to parse meal plans response: {}", e);
            anyhow::anyhow!("Invalid response format: {e}")
        })?;

        tracing::debug!("Successfully retrieved {} meal plans", meal_plans.count);
        Ok(meal_plans)
    }

    pub async fn create_meal_plan(&self, request: CreateMealPlanRequest) -> Result<MealPlan> {
        let auth_header = self.get_auth_header()?;
        let url = format!("{}/api/meal-plan/", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", auth_header)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to create meal plan: {}", response.status());
        }

        let meal_plan = response.json().await?;
        Ok(meal_plan)
    }

    pub async fn delete_meal_plan(&self, plan_id: i32) -> Result<()> {
        let auth_header = self.get_auth_header()?;
        let url = format!("{}/api/meal-plan/{}/", self.base_url, plan_id);

        let response = self
            .client
            .delete(&url)
            .header("Authorization", auth_header)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to delete meal plan: {}", response.status());
        }

        Ok(())
    }

    // Meal types
    pub async fn get_meal_types(&self) -> Result<PaginatedResponse<MealType>> {
        let auth_header = self.get_auth_header()?;
        let url = format!("{}/api/meal-type/", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", auth_header)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get meal types: {}", response.status());
        }

        let meal_types = response.json().await?;
        Ok(meal_types)
    }

    // Cook log operations
    pub async fn get_cook_log(
        &self,
        recipe_id: Option<i32>,
        days_back: Option<i32>,
    ) -> Result<PaginatedResponse<CookLog>> {
        let auth_header = self.get_auth_header()?;
        let mut url = format!("{}/api/cook-log/", self.base_url);

        let mut params = vec![];
        if let Some(recipe) = recipe_id {
            params.push(format!("recipe={recipe}"));
        }
        if let Some(days) = days_back {
            // Calculate from_date based on days_back
            let from_date = chrono::Utc::now() - chrono::Duration::days(days as i64);
            params.push(format!("from_date={}", from_date.format("%Y-%m-%d")));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        tracing::debug!(
            "Getting cook log for recipe_id: {:?}, days_back: {:?}",
            recipe_id,
            days_back
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", auth_header)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Network error getting cook log: {}", e);
                anyhow::anyhow!("Failed to connect to Tandoor API: {e}")
            })?;

        let status = response.status();
        tracing::trace!("Cook log response status: {}", status);

        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error response".to_string());
            tracing::error!(
                "Failed to get cook log with status {}: {}",
                status,
                error_body
            );
            anyhow::bail!("Failed to get cook log: {status} - {error_body}");
        }

        let cook_log: PaginatedResponse<CookLog> = response.json().await.map_err(|e| {
            tracing::error!("Failed to parse cook log response: {}", e);
            anyhow::anyhow!("Invalid response format: {e}")
        })?;

        tracing::debug!("Successfully retrieved {} cook log entries", cook_log.count);
        Ok(cook_log)
    }

    pub async fn log_cooked_recipe(&self, request: CreateCookLogRequest) -> Result<CookLog> {
        let auth_header = self.get_auth_header()?;
        let url = format!("{}/api/cook-log/", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", auth_header)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to log cooked recipe: {}", response.status());
        }

        let cook_log = response.json().await?;
        Ok(cook_log)
    }

    // Utility operations
    pub async fn get_keywords(&self) -> Result<PaginatedResponse<Keyword>> {
        let auth_header = self.get_auth_header()?;
        let url = format!("{}/api/keyword/", self.base_url);

        tracing::debug!("Making request to get keywords: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", auth_header)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Network error getting keywords: {}", e);
                anyhow::anyhow!("Failed to connect to Tandoor API: {e}")
            })?;

        let status = response.status();
        tracing::debug!("Keywords response status: {}", status);

        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error response".to_string());
            tracing::error!(
                "Failed to get keywords with status {}: {}",
                status,
                error_body
            );

            match status.as_u16() {
                401 => anyhow::bail!("Authentication expired or invalid. Please re-authenticate."),
                403 => anyhow::bail!("Access denied to keywords endpoint. Check user permissions."),
                404 => anyhow::bail!(
                    "Keywords endpoint not found. Check Tandoor version and API availability."
                ),
                500..=599 => {
                    anyhow::bail!("Tandoor server error getting keywords ({status}): {error_body}")
                }
                _ => anyhow::bail!("Failed to get keywords with status {status}: {error_body}"),
            }
        }

        let keywords: PaginatedResponse<Keyword> = response.json().await.map_err(|e| {
            tracing::error!("Failed to parse keywords response: {}", e);
            anyhow::anyhow!("Invalid response format from Tandoor server: {e}")
        })?;

        tracing::debug!("Successfully retrieved {} keywords", keywords.count);
        Ok(keywords)
    }

    // Recipe book operations
    pub async fn get_recipe_books(&self) -> Result<PaginatedResponse<RecipeBook>> {
        let auth_header = self.get_auth_header()?;
        let url = format!("{}/api/recipe-book/?page_size=100", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", auth_header)
            .send()
            .await?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to get recipe books: {body}");
        }

        let books = response.json().await?;
        Ok(books)
    }

    pub async fn create_recipe_book(&self, request: CreateRecipeBookRequest) -> Result<RecipeBook> {
        let auth_header = self.get_auth_header()?;
        let url = format!("{}/api/recipe-book/", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", auth_header)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to create recipe book: {body}");
        }

        let book = response.json().await?;
        Ok(book)
    }

    pub async fn add_recipe_to_book(
        &self,
        book_id: i32,
        recipe_id: i32,
    ) -> Result<RecipeBookEntry> {
        let auth_header = self.get_auth_header()?;
        let url = format!("{}/api/recipe-book-entry/", self.base_url);
        let request = CreateRecipeBookEntryRequest {
            book: book_id,
            recipe: recipe_id,
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", auth_header)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to add recipe to book: {body}");
        }

        let entry = response.json().await?;
        Ok(entry)
    }

    pub async fn get_units(&self) -> Result<PaginatedResponse<Unit>> {
        let auth_header = self.get_auth_header()?;
        let url = format!("{}/api/unit/", self.base_url);

        tracing::debug!("Making request to get units: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", auth_header)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Network error getting units: {}", e);
                anyhow::anyhow!("Failed to connect to Tandoor API: {e}")
            })?;

        let status = response.status();
        tracing::debug!("Units response status: {}", status);

        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error response".to_string());
            tracing::error!("Failed to get units with status {}: {}", status, error_body);

            match status.as_u16() {
                401 => anyhow::bail!("Authentication expired or invalid. Please re-authenticate."),
                403 => anyhow::bail!("Access denied to units endpoint. Check user permissions."),
                404 => anyhow::bail!(
                    "Units endpoint not found. Check Tandoor version and API availability."
                ),
                500..=599 => {
                    anyhow::bail!("Tandoor server error getting units ({status}): {error_body}")
                }
                _ => anyhow::bail!("Failed to get units with status {status}: {error_body}"),
            }
        }

        let units: PaginatedResponse<Unit> = response.json().await.map_err(|e| {
            tracing::error!("Failed to parse units response: {}", e);
            anyhow::anyhow!("Invalid response format from Tandoor server: {e}")
        })?;

        tracing::debug!("Successfully retrieved {} units", units.count);
        Ok(units)
    }
}
