//! Type definitions for the Tandoor API.
//!
//! This module contains all the data structures used for communicating with the Tandoor API,
//! including request and response types for recipes, shopping lists, meal plans, and more.
//!
//! ## Key Types
//!
//! - [`Recipe`] - Core recipe data with ingredients, steps, and metadata
//! - [`Keyword`] - Recipe categories and tags (with custom deserializer for API inconsistencies)
//! - [`ShoppingListEntry`] - Shopping list items with food, amounts, and completion status
//! - [`MealPlan`] - Scheduled meals with recipes and serving sizes
//! - [`PaginatedResponse`] - Standard API response wrapper for lists
//!
//! ## API Compatibility
//!
//! Some types include custom serialization logic to handle inconsistencies in the Tandoor API:
//! - [`Keyword`] handles both "name" and "label" fields depending on endpoint
//! - [`Recipe`] maps "created_at"/"updated_at" to "created"/"updated" fields
//! - Many fields are optional to handle varying API response formats

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// OAuth2 authentication token response from Tandoor.
///
/// Contains the Bearer token used for authenticating subsequent API requests.
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthToken {
    /// The Bearer token string (format: `tda_xxxxxxxx_xxxx_xxxx_xxxx_xxxxxxxxxxxx`)
    pub token: String,
}

/// Authentication request payload for obtaining tokens.
///
/// Used with the `/api-token-auth/` endpoint to exchange credentials for a Bearer token.
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthRequest {
    /// Tandoor username
    pub username: String,
    /// Tandoor password
    pub password: String,
}

/// A complete recipe with all its components and metadata.
///
/// Represents a recipe from the Tandoor API, including ingredients, cooking steps,
/// nutritional information, and various metadata fields.
#[derive(Debug, Serialize, Deserialize)]
pub struct Recipe {
    /// Unique recipe identifier
    pub id: i32,
    /// Recipe name/title
    pub name: String,
    /// Optional recipe description
    pub description: Option<String>,
    /// Optional cooking instructions (may be in steps instead)
    pub instructions: Option<String>,
    /// Number of servings this recipe produces
    pub servings: Option<i32>,
    /// Active cooking time in minutes
    pub working_time: Option<i32>,
    /// Passive waiting time in minutes (e.g., rising, marinating)
    pub waiting_time: Option<i32>,
    /// Recipe creation timestamp
    #[serde(rename = "created_at")]
    pub created: DateTime<Utc>,
    /// Last modification timestamp
    #[serde(rename = "updated_at")]
    pub updated: DateTime<Utc>,
    /// Whether this is an internal/system recipe
    pub internal: bool,
    /// Display setting for ingredient overview
    #[serde(default)]
    pub show_ingredient_overview: Option<bool>,
    /// Associated keywords/tags for categorization
    #[serde(default)]
    pub keywords: Vec<Keyword>,
    /// Cooking steps with ingredients and instructions
    #[serde(default)]
    pub steps: Vec<Step>,
    /// Nutritional information if calculated
    #[serde(default)]
    pub nutrition: Option<Nutrition>,
    /// Recipe image URL or path
    #[serde(default)]
    pub image: Option<String>,
    /// User who created this recipe (may be an object or integer)
    #[serde(default)]
    pub created_by: Option<serde_json::Value>,
    /// Original source URL if imported
    #[serde(default)]
    pub source_url: Option<String>,
    /// Additional recipe properties
    #[serde(default)]
    pub properties: Option<Vec<serde_json::Value>>,
    /// Food-specific properties
    #[serde(default)]
    pub food_properties: Option<serde_json::Value>,
    /// File system path for recipe data
    #[serde(default)]
    pub file_path: Option<String>,
    /// Human-readable servings description
    #[serde(default)]
    pub servings_text: Option<String>,
    /// User rating (1-5 stars)
    #[serde(default)]
    pub rating: Option<f64>,
    /// When this recipe was last cooked
    #[serde(default)]
    pub last_cooked: Option<DateTime<Utc>>,
    /// Whether this recipe is private to the creator
    #[serde(default)]
    pub private: Option<bool>,
    /// Users/groups this recipe is shared with
    #[serde(default)]
    pub shared: Option<Vec<serde_json::Value>>,
    /// Whether this recipe is newly added
    #[serde(default)]
    pub new: Option<bool>,
}

/// Recipe keyword/tag for categorization and filtering.
///
/// Keywords are used to categorize recipes and can be hierarchical.
/// This type includes a custom deserializer to handle API inconsistencies
/// where some endpoints return "name" and others return "label".
#[derive(Debug, Serialize)]
pub struct Keyword {
    /// Unique keyword identifier
    pub id: i32,
    /// Keyword name (primary field)
    pub name: String,
    /// Alternative label field (API compatibility)
    #[serde(default)]
    pub label: Option<String>,
    /// Optional keyword description
    #[serde(default)]
    pub description: Option<String>,
    /// Creation timestamp
    #[serde(rename = "created_at", default)]
    pub created: Option<DateTime<Utc>>,
    /// Last modification timestamp
    #[serde(rename = "updated_at", default)]
    pub updated: Option<DateTime<Utc>>,
    /// Parent keyword for hierarchical organization
    #[serde(default)]
    pub parent: Option<serde_json::Value>,
    /// Number of child keywords
    #[serde(default)]
    pub numchild: Option<i32>,
    /// Full hierarchical name path
    #[serde(default)]
    pub full_name: Option<String>,
}

/// Custom deserializer for Keyword to handle API inconsistencies.
///
/// The Tandoor API returns keywords with different field names depending on the endpoint:
/// - Search endpoints use "label"
/// - Detail endpoints use "name"
impl<'de> serde::Deserialize<'de> for Keyword {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct KeywordVisitor;

        impl<'de> Visitor<'de> for KeywordVisitor {
            type Value = Keyword;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a keyword object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut id = None;
                let mut name = None;
                let mut label = None;
                let mut description = None;
                let mut created = None;
                let mut updated = None;
                let mut parent = None;
                let mut numchild = None;
                let mut full_name = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "id" => id = Some(map.next_value()?),
                        "name" => name = Some(map.next_value()?),
                        "label" => label = Some(map.next_value()?),
                        "description" => description = map.next_value()?,
                        "created_at" => created = map.next_value()?,
                        "updated_at" => updated = map.next_value()?,
                        "parent" => parent = map.next_value()?,
                        "numchild" => numchild = map.next_value()?,
                        "full_name" => full_name = map.next_value()?,
                        _ => {
                            let _ = map.next_value::<serde_json::Value>()?;
                        }
                    }
                }

                let id = id.ok_or_else(|| de::Error::missing_field("id"))?;

                // Use name if present, otherwise use label (handles API inconsistency)
                let name = name
                    .or_else(|| label.clone())
                    .ok_or_else(|| de::Error::missing_field("name or label"))?;

                Ok(Keyword {
                    id,
                    name,
                    label,
                    description,
                    created,
                    updated,
                    parent,
                    numchild,
                    full_name,
                })
            }
        }

        deserializer.deserialize_map(KeywordVisitor)
    }
}

/// A single cooking step within a recipe.
///
/// Steps contain ingredients and instructions, and can be ordered sequentially.
#[derive(Debug, Serialize, Deserialize)]
pub struct Step {
    /// Unique step identifier
    pub id: i32,
    /// Step name/title
    pub name: String,
    /// Cooking instruction text
    pub instruction: String,
    /// Ingredients needed for this step
    pub ingredients: Vec<StepIngredient>,
    /// Time required for this step in minutes
    pub time: Option<i32>,
    /// Display order within the recipe
    pub order: i32,
    /// Associated media file (image/video)
    pub file: Option<String>,
    /// Markdown-formatted instructions
    #[serde(default)]
    pub instructions_markdown: Option<String>,
    /// Whether to show this step as a section header
    #[serde(default)]
    pub show_as_header: Option<bool>,
    /// Reference to a sub-recipe
    #[serde(default)]
    pub step_recipe: Option<i32>,
    /// Sub-recipe data if applicable
    #[serde(default)]
    pub step_recipe_data: Option<serde_json::Value>,
    /// Whether to show ingredients in table format
    #[serde(default)]
    pub show_ingredients_table: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StepIngredient {
    pub id: i32,
    pub food: Food,
    pub unit: Option<Unit>,
    pub amount: f64,
    pub note: Option<String>,
    pub order: i32,
    pub is_header: bool,
    pub no_amount: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Food {
    pub id: i32,
    pub name: String,
    pub plural_name: Option<String>,
    pub description: Option<String>,
    pub recipe: Option<serde_json::Value>,
    pub food_onhand: bool,
    pub supermarket_category: Option<serde_json::Value>,
    pub inherit_fields: Vec<InheritField>,
    pub properties: Vec<FoodProperty>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Unit {
    pub id: i32,
    pub name: String,
    pub plural_name: Option<String>,
    pub description: Option<String>,
    pub base_unit: Option<String>,
    #[serde(rename = "type", default)]
    pub type_: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InheritField {
    pub id: i32,
    pub name: String,
    pub field: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FoodProperty {
    pub id: i32,
    pub property_amount: f64,
    pub property_type: PropertyType,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PropertyType {
    pub id: i32,
    pub name: String,
    pub unit: String,
    pub order: i32,
    pub fdc_id: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Nutrition {
    pub calories: Option<f64>,
    pub proteins: Option<f64>,
    pub fats: Option<f64>,
    pub carbs: Option<f64>,
}

/// An entry in a shopping list with food item and purchase details.
#[derive(Debug, Serialize, Deserialize)]
pub struct ShoppingListEntry {
    /// Unique entry identifier
    pub id: i32,
    /// Food item to purchase
    pub food: Food,
    /// Measurement unit (grams, cups, etc.)
    pub unit: Option<Unit>,
    /// Quantity to purchase
    pub amount: f64,
    /// Display order in the shopping list
    pub order: i32,
    /// Whether this item has been purchased
    pub checked: bool,
    /// When this entry was added
    pub created: DateTime<Utc>,
    /// When this item was marked as purchased
    pub completed: Option<DateTime<Utc>>,
    /// Optional delay before showing this item
    pub delay_until: Option<DateTime<Utc>>,
    /// User who added this entry (may be an object or integer)
    pub created_by: serde_json::Value,
    /// User who marked this as completed (may be an object or integer)
    pub completed_by: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MealPlan {
    pub id: i32,
    pub title: Option<String>,
    pub recipe: Option<Recipe>,
    pub servings: i32,
    pub note: Option<String>,
    pub date: chrono::NaiveDate,
    pub meal_type: MealType,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub created_by: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MealType {
    pub id: i32,
    pub name: String,
    pub order: i32,
    pub color: String,
    pub default: bool,
    pub created_by: serde_json::Value,
    pub icon: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CookLog {
    pub id: i32,
    pub recipe: Recipe,
    pub servings: i32,
    pub rating: Option<i32>,
    pub comment: Option<String>,
    pub created: DateTime<Utc>,
    pub created_by: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecipeImport {
    pub url: String,
}

/// Standard paginated response wrapper used by most Tandoor API endpoints.
///
/// This follows Django REST framework pagination format.
#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    /// Total number of items across all pages
    pub count: i32,
    /// URL for the next page of results
    pub next: Option<String>,
    /// URL for the previous page of results
    pub previous: Option<String>,
    /// Items for the current page
    pub results: Vec<T>,
}

// Request types for creating/updating resources
// These types are used when sending data to the API to create or modify entities.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRecipeRequest {
    pub name: String,
    pub description: Option<String>,
    pub servings: Option<i32>,
    pub working_time: i32,
    pub waiting_time: i32,
    pub keywords: Vec<CreateKeywordRequest>,
    pub steps: Vec<CreateStepRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateKeywordRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateStepRequest {
    pub name: Option<String>,
    pub instruction: String,
    pub ingredients: Vec<CreateStepIngredientRequest>,
    pub time: Option<i32>,
    pub order: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateStepIngredientRequest {
    pub food: CreateFoodRequest,
    pub unit: Option<CreateUnitRequest>,
    pub amount: String,
    pub note: Option<String>,
    pub order: i32,
    pub is_header: bool,
    pub no_amount: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateFoodRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUnitRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateShoppingListEntryRequest {
    pub food: i32,
    pub unit: Option<i32>,
    pub amount: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateMealPlanRequest {
    pub recipe: Option<i32>,
    pub title: Option<String>,
    pub servings: i32,
    pub date: chrono::NaiveDate,
    pub meal_type: i32,
    pub note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCookLogRequest {
    pub recipe: i32,
    pub servings: i32,
    pub rating: Option<i32>,
    pub comment: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateFoodRequest {
    pub food_onhand: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateShoppingListEntryRequest {
    pub checked: Option<bool>,
    pub amount: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct BulkShoppingListRequest {
    pub entries: Vec<CreateShoppingListEntryRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateRecipeKeywordsRequest {
    pub keywords: Vec<CreateKeywordRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecipeBook {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub created_by: serde_json::Value,
    #[serde(default)]
    pub filter: Option<serde_json::Value>,
    #[serde(default)]
    pub order: Option<i32>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecipeBookEntry {
    pub id: i32,
    pub book: i32,
    pub book_content: Option<serde_json::Value>,
    pub recipe: i32,
    pub recipe_content: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRecipeBookRequest {
    pub name: String,
    pub description: String,
    pub shared: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRecipeBookEntryRequest {
    pub book: i32,
    pub recipe: i32,
}
