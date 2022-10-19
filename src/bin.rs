use std::env;

use discord_xiv_emotes::setup_client;
use dotenv::dotenv;
use sqlx::PgPool;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("expected DISCORD_TOKEN env var");
    let pool = PgPool::connect(&env::var("DATABASE_URL").expect("expected DATABASE_URL env var"))
        .await
        .expect("could not connect to database");
    let mut client = setup_client(token, pool).await;

    client.start().await.expect("couldn't start client");
}
