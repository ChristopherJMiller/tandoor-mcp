# Tandoor Recipes MCP Server

A Model Context Protocol (MCP) server for [Tandoor Recipes](https://tandoor.dev) that provides comprehensive recipe management, shopping lists, meal planning, and inventory tracking capabilities.

## Features

- **Recipe Management**: Search, create, and manage recipes
- **Shopping Lists**: Add items, mark as purchased, and manage shopping workflows
- **Meal Planning**: Plan meals and manage meal schedules
- **Inventory Tracking**: Monitor pantry items and get recipe suggestions
- **Keywords & Tags**: Organize recipes with keywords and categories
- **Cooking Logs**: Track cooking history and ratings

## Quick Start

1. **Clone the repository**:

```bash
git clone https://github.com/your-repo/mcp-tandoor
cd mcp-tandoor
```

2. **Configure environment variables**:

```bash
export TANDOOR_BASE_URL="http://localhost:8080"
export TANDOOR_USERNAME="admin"
export TANDOOR_PASSWORD="your-password"
```

3. **Run the server**:

rustup is recommended:

```bash
cargo run
```

## Tandoor Configuration

⚠️ **IMPORTANT**: Tandoor uses a multi-tenant permission system that requires specific setup for API access to work properly. Without proper space and group configuration, you'll get permission errors even with valid authentication.

### Method 1: Web Interface Setup (Recommended)

1. **Access Tandoor admin interface**:

   - Go to `http://your-tandoor-url/admin/`
   - Login with superuser credentials

2. **Create/Verify Groups**:

   - Navigate to **Authentication and Authorization** → **Groups**
   - Ensure these groups exist: `admin`, `user`, `guest`
   - If missing, create them (names must match exactly)

3. **Create a Space**:

   - Navigate to **Cookbook** → **Spaces**
   - Click **Add Space**
   - Fill in:
     - **Name**: Your organization/space name (e.g., "Production", "Family")
     - **Max recipes**: 0 (unlimited)
     - **Max users**: 0 (unlimited)
     - **Max file storage mb**: 0 (unlimited)
     - **Allow sharing**: Checked
   - Click **Save**

4. **Create User Space Association**:
   - Navigate to **Cookbook** → **User spaces**
   - Click **Add User space**
   - Select:
     - **User**: Your admin user
     - **Space**: The space you just created
     - **Active**: Must be checked
   - Click **Save**
   - After saving, click on the created User space entry
   - In **Groups**, select `admin` and add it
   - Click **Save**

### Method 2: Command Line Setup

If you prefer command line or don't have web admin access:

```bash
# Enter your Tandoor container
docker exec -it your-tandoor-container /opt/recipes/venv/bin/python manage.py shell

# Run this Python code:
from cookbook.models import Space, UserSpace
from django.contrib.auth.models import User, Group

# Get your admin user (replace 'admin' with your username)
admin_user = User.objects.get(username='admin')

# Create a space
space, created = Space.objects.get_or_create(
    name='Production',  # Choose your space name
    defaults={
        'created_by': admin_user,
        'max_recipes': 0,
        'max_users': 0,
        'max_file_storage_mb': 0,
        'allow_sharing': True
    }
)

# Associate user with space
user_space, created = UserSpace.objects.get_or_create(
    user=admin_user,
    space=space,
    defaults={'active': True}
)

# Ensure it's active if it already existed
if not created and not user_space.active:
    user_space.active = True
    user_space.save()

# Add user to admin group in this space
admin_group = Group.objects.get(name='admin')
user_space.groups.add(admin_group)
```

### Verification

Test that your setup is working:

```bash
# Get an API token
curl -X POST http://your-tandoor-url/api-token-auth/ \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"your-password"}'

# Test API access
curl -X GET http://your-tandoor-url/api/keyword/ \
  -H "Authorization: Bearer <YOUR_TOKEN>"
```

You should get a JSON response with keywords

## Configuration

### Environment Variables

| Variable           | Description                         | Default                 |
| ------------------ | ----------------------------------- | ----------------------- |
| `TANDOOR_BASE_URL` | Full URL to your Tandoor instance   | `http://localhost:8080` |
| `TANDOOR_USERNAME` | Username for Tandoor authentication | `admin`                 |
| `TANDOOR_PASSWORD` | Password for Tandoor authentication | `admin`                 |
| `BIND_ADDR`        | Address and port for the MCP server | `127.0.0.1:3001`        |

### Runtime Configuration

```bash
# Example with custom configuration
TANDOOR_BASE_URL="https://recipes.mycompany.com" \
TANDOOR_USERNAME="api-user" \
TANDOOR_PASSWORD="secure-password" \
BIND_ADDR="0.0.0.0:8000" \
cargo run
```

## Available Tools

The MCP server provides the following tools:

### Recipe Management

- `search_recipes` - Search for recipes with flexible querying
- `get_recipe_details` - Get comprehensive recipe information with scaled ingredients
- `create_recipe` - Create a new recipe
- `import_recipe_from_url` - Import a recipe from an external URL

### Shopping Lists

- `add_to_shopping_list` - Add items to shopping list with intelligent consolidation
- `get_shopping_list` - Get current shopping list organized by store section
- `check_shopping_items` - Mark shopping list items as checked/purchased
- `clear_shopping_list` - Clear checked items from shopping list and update pantry

### Food & Inventory

- `search_foods` - Search for foods/ingredients with fuzzy name matching
- `update_pantry` - Update pantry inventory status
- `suggest_from_inventory` - Get recipe suggestions based on current inventory

### Meal Planning

- `get_meal_plans` - Get meal plans for a date range
- `create_meal_plan` - Create a new meal plan
- `delete_meal_plan` - Delete a meal plan
- `get_meal_types` - Get available meal types

### Metadata

- `get_keywords` - Get all available recipe keywords/tags across every API page
  (see [MCP tool behavior](docs/mcp.md#keyword-listing))
- `get_units` - Get available measurement units

### Cooking History

- `get_cook_log` - Get cooking history
- `log_cooked_recipe` - Log a cooked recipe

## Troubleshooting

### "Authentication credentials were not provided"

This indicates the OAuth2 token format is wrong. Ensure you're using:

- `Authorization: Bearer YOUR_TOKEN`
- Token from the `/api-token-auth/` endpoint

### "You do not have permission to perform this action"

This is a Tandoor permissions issue:

1. **Check user is in a group**:

```python
user = User.objects.get(username='your-username')
print(f"Groups: {[g.name for g in user.groups.all()]}")
```

2. **Check UserSpace is active**:

```python
user_spaces = UserSpace.objects.filter(user=user)
for us in user_spaces:
    print(f"Space: {us.space.name}, Active: {us.active}")
```

3. **Most common fix**:

```python
# Make sure UserSpace is active
user_space = UserSpace.objects.get(user=user)
user_space.active = True
user_space.save()
```

### "Request was throttled"

Tandoor has rate limiting. Wait before retrying, or check if you're making too many authentication requests. The MCP server implements shared authentication to prevent this.

### Connection Refused / Network Errors

- Verify `TANDOOR_BASE_URL` is correct and accessible
- Check if Tandoor is running: `curl http://your-tandoor-url/`
- Verify firewall/network configuration

### Django Scopes Error

```
ScopeError: A scope on dimension(s) space needs to be active for this query
```

This means the space scope isn't activated. Follow the Tandoor Configuration section above to fix permissions.

## Development

### Building from Source

```bash
# Clone the repository
git clone https://github.com/your-repo/mcp-tandoor
cd mcp-tandoor

# Build in development mode
cargo build

# Run with debug logging
RUST_LOG=debug cargo run
```

### Running Tests

The project includes a comprehensive testing script that manages Docker services and runs integration tests:

```bash
# Run full test suite (clean start, run tests, cleanup)
./scripts/test.sh test

# Keep services running after tests (for debugging)
./scripts/test.sh test --keep-running

# Run tests with verbose output
./scripts/test.sh test --verbose

# Manual service management
./scripts/test.sh up      # Start Tandoor services
./scripts/test.sh down    # Stop services
./scripts/test.sh reset   # Clean and restart services
./scripts/test.sh clean   # Clean volumes only
./scripts/test.sh logs    # View service logs
```

**Testing Requirements:**

- Docker and Docker Compose installed
- Ports 8080 (Tandoor) and 5432 (PostgreSQL) available
- Sufficient disk space for Docker volumes

**Note on Rate Limiting:** Tandoor limits authentication to 10 requests per day. The test script automatically obtains and shares a single token across all tests to avoid hitting this limit.

## Rate Limiting Considerations

Tandoor has extremely aggressive rate limiting on authentication endpoints:

- **10 authentication requests per day per IP address**
- Rate limit resets at midnight UTC or when Tandoor is restarted
- Exceeding the limit returns `429 Too Many Requests`

### Handling Rate Limits during Verification

1. **Token Sharing**: Uses global token storage to share authentication across all tool calls
2. **Environment Token**: Accepts `TANDOOR_AUTH_TOKEN` to bypass authentication entirely
3. **Graceful Fallback**: Falls back to stored credentials if token expires

### Best Practices

```bash
# Get a token once and reuse it
TOKEN=$(curl -X POST http://localhost:8080/api-token-auth/ \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"your-password"}' | jq -r .token)

# Use the token for all subsequent requests
TANDOOR_AUTH_TOKEN="$TOKEN" cargo run
```

### Development Workflow

```bash
# Start Tandoor and get a fresh token
./scripts/test.sh up
TOKEN=$(./scripts/get_token.sh admin testing1 http://localhost:8080)

# Use the token for development
TANDOOR_AUTH_TOKEN="$TOKEN" cargo run

# When done for the day
./scripts/test.sh down
```

### If You Hit the Rate Limit

1. **Wait until midnight UTC** (rate limit resets)
2. **Restart Tandoor** to reset the counter:
   ```bash
   docker restart your-tandoor-container
   ```
3. **Use an existing token** if you have one saved
