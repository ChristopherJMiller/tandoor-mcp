# Tandoor MCP

An MCP server that connects AI assistants to [Tandoor Recipes](https://tandoor.dev) — search recipes, manage shopping lists, plan meals, and track inventory through natural conversation.

Built in Rust. Single binary, no runtime dependencies.

## What It Does

Talk to your Tandoor instance through any MCP-compatible client (Claude Desktop, Claude Code, etc.):

- **Recipes** — search, view details with scaled ingredients, create new ones, organize with tags and books
- **Shopping lists** — add items (manually or from recipes), check off purchases, sync to pantry
- **Meal planning** — schedule meals, browse plans by date range
- **Inventory** — track what's in your pantry, get recipe suggestions based on available ingredients
- **Cooking log** — record what you cooked, with ratings and comments

## Setup

### 1. Get the server

**Linux / WSL** — download the prebuilt binary from the [Releases page](https://github.com/iosue-iulianus/tandoor-mcp/releases):

```bash
chmod +x mcp-tandoor-linux-x86_64
./mcp-tandoor-linux-x86_64
```

**macOS / other platforms / build from source** (requires [Rust 1.85+](https://rustup.rs)):

```bash
# Install Rust (if you don't have it)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# Linux only — install OpenSSL dev headers
sudo apt install pkg-config libssl-dev    # Debian/Ubuntu
sudo dnf install pkg-config openssl-devel # Fedora/RHEL
sudo pacman -S pkg-conf openssl           # Arch

# Build and run
git clone https://github.com/iosue-iulianus/tandoor-mcp.git
cd tandoor-mcp
cargo build --release
./target/release/mcp-tandoor
```

### 2. Configure

```bash
cp .env.example .env
# Edit .env with your Tandoor URL and credentials
```

| Variable | Description | Default |
|----------|-------------|---------|
| `TANDOOR_BASE_URL` | Your Tandoor instance URL | `http://localhost:8080` |
| `TANDOOR_USERNAME` | Tandoor username | `admin` |
| `TANDOOR_PASSWORD` | Tandoor password | `admin` |
| `BIND_ADDR` | Server listen address | `127.0.0.1:3001` |
| `RUST_LOG` | Log level (`info`, `debug`, `trace`) | `info` |

The server automatically authenticates with your username and password on startup and caches the API token for the session.

> **Note:** Tandoor limits login attempts to 10 per day. Normal usage is unaffected since the server only logs in once at startup. If you're restarting the server frequently and hit the limit, you can set `TANDOOR_AUTH_TOKEN` directly in `.env` to skip the login step.

### 3. Connect your MCP client

**Claude Desktop** — add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "tandoor": {
      "url": "http://127.0.0.1:3001/sse"
    }
  }
}
```

**Claude Code:**

```bash
claude mcp add tandoor --transport sse http://127.0.0.1:3001/sse
```

## Tools

| Tool | Description |
|------|-------------|
| `search_recipes` | Search recipes with flexible querying and pagination |
| `get_recipe_details` | Full recipe info with scaled ingredients |
| `create_recipe` | Create a new recipe with instructions, times, and tags |
| `update_recipe_keywords` | Set or append tags on a recipe |
| `get_recipe_books` | List recipe books/collections |
| `create_recipe_book` | Create a new recipe book |
| `add_recipe_to_book` | Add a recipe to a book |
| `add_to_shopping_list` | Add items manually or from a recipe |
| `get_shopping_list` | View current shopping list |
| `check_shopping_items` | Mark items as purchased |
| `clear_shopping_list` | Clear checked items and update pantry |
| `search_foods` | Fuzzy search for foods/ingredients |
| `update_pantry` | Update pantry inventory status |
| `suggest_from_inventory` | Recipe suggestions from what you have on hand |
| `get_meal_plans` | View meal plans for a date range |
| `create_meal_plan` | Schedule a meal |
| `delete_meal_plan` | Remove a meal plan entry |
| `get_meal_types` | List available meal type categories |
| `get_keywords` | List all recipe tags |
| `get_units` | List measurement units |
| `get_cook_log` | View cooking history |
| `log_cooked_recipe` | Log a cooked recipe with rating and comments |

## Tandoor Permissions

Tandoor uses a multi-tenant Space system. Your API user needs to be assigned to a Space with the `admin` group role and marked as **Active**.

The quickest way: go to `http://your-tandoor/admin/` and ensure your user has a UserSpace entry with an active admin group assignment. See the [Tandoor docs](https://docs.tandoor.dev) for details.

## Troubleshooting

| Problem | Fix |
|---------|-----|
| "Authentication credentials were not provided" | Token missing or malformed — use `Bearer YOUR_TOKEN` |
| "You do not have permission" | User not assigned to an active Space with a group role |
| "Request was throttled" / 429 | Hit the 10 login/day limit — switch to `TANDOOR_AUTH_TOKEN` |
| `ScopeError: ...space needs to be active` | UserSpace doesn't exist or `active` is false |

## Development

```bash
RUST_LOG=debug cargo run          # Run with debug logging

./scripts/test.sh up              # Start local Tandoor (Docker)
./scripts/test.sh test            # Run tests
./scripts/test.sh down            # Stop local Tandoor
```

## Acknowledgments

Originally forked from [ChristopherJMiller/tandoor-mcp](https://github.com/ChristopherJMiller/tandoor-mcp).

## License

MIT
