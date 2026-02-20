use anyhow::Result;
use mcp_tandoor::TandoorClient;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

#[allow(dead_code)]
pub struct TestEnvironment {
    pub client: TandoorClient,
}

// Global token storage to share across all tests
#[allow(dead_code)]
static SHARED_TOKEN: OnceLock<String> = OnceLock::new();

impl TestEnvironment {
    #[allow(dead_code)]
    pub async fn new() -> Result<Self> {
        let base_url = std::env::var("TANDOOR_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:8080".to_string());
        let username = std::env::var("TANDOOR_USERNAME").unwrap_or_else(|_| "admin".to_string());
        let password = std::env::var("TANDOOR_PASSWORD").unwrap_or_else(|_| "testing1".to_string());

        let mut client = TandoorClient::new(base_url.clone());

        // Check if we have a shared token first
        if let Some(token) = SHARED_TOKEN.get() {
            println!("Using shared token from previous test");
            client.set_token(token.clone());
            return Ok(Self { client });
        }

        // Try to get token from environment variable (if set by test script)
        if let Ok(token) = std::env::var("TANDOOR_AUTH_TOKEN") {
            println!("Using token from TANDOOR_AUTH_TOKEN environment variable");
            client.set_token(token.clone());
            let _ = SHARED_TOKEN.set(token);
            return Ok(Self { client });
        }

        // Last resort: try to authenticate (may fail due to rate limiting)
        println!("Warning: Attempting authentication. This may fail due to rate limiting (10 requests/day)!");
        println!("Consider setting TANDOOR_AUTH_TOKEN environment variable with a valid token.");

        match client
            .authenticate(username.clone(), password.clone())
            .await
        {
            Ok(_) => {
                println!("Authentication successful - storing token for other tests");
                if let Some(token) = client.get_token() {
                    let _ = SHARED_TOKEN.set(token.to_string());
                }
                Ok(Self { client })
            }
            Err(e) => {
                eprintln!("Authentication failed: {e}");
                eprintln!("\nTo fix this:");
                eprintln!("1. Wait for rate limit to reset (24 hours)");
                eprintln!("2. Or get a token manually and set TANDOOR_AUTH_TOKEN");
                eprintln!("3. Or restart Tandoor to reset rate limits");
                Err(e)
            }
        }
    }
}

pub struct DockerEnvironment;

impl DockerEnvironment {
    pub fn is_running() -> bool {
        let output = Command::new("docker")
            .args(["compose", "ps", "-q", "web_recipes"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output();

        match output {
            Ok(result) => !result.stdout.is_empty(),
            Err(_) => false,
        }
    }

    pub fn ensure_running() -> Result<()> {
        if !Self::is_running() {
            anyhow::bail!(
                "Tandoor Docker services are not running. Please run: ./scripts/test.sh up"
            );
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! setup_test {
    () => {
        async {
            use $crate::common::{DockerEnvironment, TestEnvironment};

            // Ensure Docker is running
            DockerEnvironment::ensure_running().expect("Docker environment check failed");

            // Set up test environment
            TestEnvironment::new()
                .await
                .expect("Failed to create test environment")
        }
    };
}

pub fn init_test_logging() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(true)
        .try_init();
}
