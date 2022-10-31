use std::env;

use discord_xiv_emotes::setup_client;
use dotenvy::dotenv;
use sqlx::PgPool;
use tracing::*;
use tracing_subscriber::EnvFilter;

#[tokio::main]
#[instrument]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt()
        .pretty()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    let token = env::var("DISCORD_TOKEN").expect("expected DISCORD_TOKEN env var");
    let db_url = env::var("DATABASE_URL").expect("expected DATABASE_URL env var");
    let pool = PgPool::connect(&db_url)
        .await
        .expect("could not connect to database");
    info!("connected to db at {}", db_url);
    let mut client = setup_client(token, pool).await;

    client.start().await.expect("couldn't start client");
}
