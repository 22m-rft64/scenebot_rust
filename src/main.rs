use dotenvy::dotenv;
use env_logger::init;
use log::error;
use mongodb::{
    options::{ClientOptions, ResolverConfig},
    Client as DBClient,
};
use serenity::{prelude::*, Error as SerenityError};

mod command_handler;
mod event_handler;
mod notice_handler;
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

    // 안내 모드: DB, privileged intent 없이 명령어 응답만 함
    if env::var("NOTICE_MODE").is_ok_and(|v| v == "1") {
        let mut client = Client::builder(&token, GatewayIntents::GUILDS)
            .event_handler(notice_handler::NoticeHandler)
            .await
            .expect("Err creating client.");
        return run_client(&mut client).await;
    }

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

    run_client(&mut client).await
}

// 토큰/intent 문제는 재시작해도 안 고쳐짐. 프로세스 죽으면 docker가 1분마다 재접속시켜서
// 디코가 토큰 리셋해버림 (2026-10-10 이렘). 그래서 이런 에러면 안 죽고 그냥 대기.
async fn run_client(client: &mut Client) -> Result<(), Box<dyn Error>> {
    use serenity::gateway::GatewayError;

    match client.start().await {
        Ok(()) => Ok(()),
        Err(SerenityError::Gateway(
            why @ (GatewayError::InvalidAuthentication
            | GatewayError::DisallowedGatewayIntents
            | GatewayError::InvalidGatewayIntents),
        )) => {
            error!("fatal gateway error, idling until manually restarted: {:?}", why);
            std::future::pending::<()>().await;
            Ok(())
        }
        Err(why) => Err(why.into()),
    }
}
