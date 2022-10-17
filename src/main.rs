use std::env;

use discord_xiv_emotes::setup_client;
use dotenv::dotenv;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("expected DISCORD_TOKEN env var");
    let mut client = setup_client(token).await;

    client.start().await.expect("couldn't start client");
}
