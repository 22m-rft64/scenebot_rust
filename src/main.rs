use dotenvy::dotenv;
use env_logger::init;
use mongodb::{
    options::{ClientOptions, ResolverConfig},
    Client as DBClient,
};
use serenity::prelude::*;

mod command_handler;
mod event_handler;
mod events;
mod utils;

use std::{collections::HashMap, env, error::Error, num::NonZeroU64, sync::Arc};
use tokio::sync::{Mutex, RwLock};

pub struct GlobalGuildConfigs;
impl TypeMapKey for GlobalGuildConfigs {
    type Value =
        Arc<RwLock<HashMap<NonZeroU64, Arc<Mutex<crate::utils::guild_config::GuildConfig>>>>>;
}

pub struct DatabaseClient;
impl TypeMapKey for DatabaseClient {
    type Value = mongodb::Client;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    init();

    let token = env::var("DISCORD_BOT_TOKEN").expect("DISCORD_BOT_TOKEN is not set");

    // Fail fast if BOT_DB_NAME is missing — used everywhere with .unwrap() downstream.
    env::var("BOT_DB_NAME").expect("BOT_DB_NAME is not set");

    let intents =
        GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let client_uri = env::var("DB_URI").expect("couldn't find db uri");
    let options =
        ClientOptions::parse_with_resolver_config(&client_uri, ResolverConfig::cloudflare())
            .await?;

    let db_client = DBClient::with_options(options)?;

    let handler = event_handler::DiscordEventHandler {
        database: db_client.clone(),
    };

    let mut client = Client::builder(&token, intents)
        .event_handler(handler)
        .await
        .expect("Err creating client.");

    {
        let mut data = client.data.write().await;
        data.insert::<GlobalGuildConfigs>(Arc::new(RwLock::new(HashMap::default())));
        data.insert::<DatabaseClient>(db_client);
    }

    client.start().await?;
    Ok(())
}
