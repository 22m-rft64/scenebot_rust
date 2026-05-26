// 일회성 마이그레이션 스크립트:
//   1) DISCORD_BOT_TOKEN으로 GET /users/@me/guilds → 진짜 i64 guild_id 수집
//   2) 각 guild_id를 f64로 변환해 mongodb에서 매칭되는 doc 검색
//   3) 매칭되면 새 i64 doc으로 insert + 옛 f64 doc 삭제
//
// 사용:
//   cd scenebot_rust
//   .env 준비 (DISCORD_BOT_TOKEN, DB_URI, BOT_DB_NAME)
//   cargo run --release --bin migrate_guild_ids
//
// 멱등(idempotent): 이미 i64로 옮긴 doc은 건너뜀.

use dotenvy::dotenv;
use env_logger::init;
use log::{error, info, warn};
use mongodb::{
    bson::{doc, Document},
    options::{ClientOptions, ResolverConfig},
    Client as DBClient,
};
use serde::Deserialize;
use std::{env, error::Error};

#[derive(Deserialize)]
struct PartialGuild {
    id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    init();

    let token = env::var("DISCORD_BOT_TOKEN").expect("DISCORD_BOT_TOKEN is not set");
    let client_uri = env::var("DB_URI").expect("DB_URI is not set");
    let db_name = env::var("BOT_DB_NAME").expect("BOT_DB_NAME is not set");

    info!("fetching guild list from Discord API...");
    let guild_ids = fetch_all_guilds(&token).await?;
    info!("Discord reports {} guilds for this bot", guild_ids.len());

    let options =
        ClientOptions::parse_with_resolver_config(&client_uri, ResolverConfig::cloudflare())
            .await?;
    let db_client = DBClient::with_options(options)?;
    let collection: mongodb::Collection<Document> =
        db_client.database("scene").collection(&db_name);

    let mut migrated = 0usize;
    let mut already_i64 = 0usize;
    let mut not_found = 0usize;
    let mut errors = 0usize;

    for id_str in &guild_ids {
        let id: u64 = match id_str.parse() {
            Ok(n) => n,
            Err(why) => {
                error!("invalid guild id {}: {:?}", id_str, why);
                errors += 1;
                continue;
            }
        };
        let i64_id = id as i64;
        let f64_id = id as f64;

        // 1) 이미 i64로 저장돼있으면 skip.
        match collection
            .find_one(doc! { "guild_id": i64_id }, None)
            .await
        {
            Ok(Some(_)) => {
                already_i64 += 1;
                continue;
            }
            Ok(None) => {}
            Err(why) => {
                error!("query (i64) failed for guild {}: {:?}", id, why);
                errors += 1;
                continue;
            }
        }

        // 2) f64 legacy doc 검색 후 i64로 교체.
        match collection
            .find_one(doc! { "guild_id": f64_id }, None)
            .await
        {
            Ok(Some(legacy)) => {
                let auto_enable = legacy.get_bool("auto_magnitute_enable").unwrap_or(false);
                let auto_config = legacy
                    .get_str("auto_magnitute_config")
                    .unwrap_or("Auto")
                    .to_string();

                if let Err(why) = collection
                    .delete_one(doc! { "guild_id": f64_id }, None)
                    .await
                {
                    warn!("delete f64 doc failed (guild {}): {:?}", id, why);
                }
                if let Err(why) = collection
                    .insert_one(
                        doc! {
                            "guild_id": i64_id,
                            "auto_magnitute_enable": auto_enable,
                            "auto_magnitute_config": auto_config.clone(),
                        },
                        None,
                    )
                    .await
                {
                    error!("insert i64 doc failed (guild {}): {:?}", id, why);
                    errors += 1;
                    continue;
                }
                info!(
                    "migrated guild {} (auto_enable={}, config={})",
                    id, auto_enable, auto_config
                );
                migrated += 1;
            }
            Ok(None) => {
                info!(
                    "guild {} has no DB record yet (봇 부팅 시 새 doc으로 생성됨)",
                    id
                );
                not_found += 1;
            }
            Err(why) => {
                error!("query (f64) failed for guild {}: {:?}", id, why);
                errors += 1;
            }
        }
    }

    info!("=== migration done ===");
    info!(
        "migrated: {}, already_i64: {}, not_found: {}, errors: {}",
        migrated, already_i64, not_found, errors
    );

    Ok(())
}

async fn fetch_all_guilds(token: &str) -> Result<Vec<String>, Box<dyn Error>> {
    use log::warn;
    use tokio::time::{sleep, Duration};

    let client = reqwest::Client::new();
    let mut all_guilds: Vec<String> = Vec::new();
    let mut after: Option<String> = None;

    loop {
        let mut url = String::from("https://discord.com/api/v10/users/@me/guilds?limit=200");
        if let Some(a) = &after {
            url.push_str(&format!("&after={}", a));
        }

        // Discord rate-limit retry loop.
        let guilds: Vec<PartialGuild> = loop {
            let resp = client
                .get(&url)
                .header("Authorization", format!("Bot {}", token))
                .send()
                .await?;

            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let retry_after_secs: f64 = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(2.0);
                let sleep_ms = (retry_after_secs * 1000.0) as u64 + 250;
                warn!(
                    "Discord rate-limited, sleeping {}ms before retry",
                    sleep_ms
                );
                sleep(Duration::from_millis(sleep_ms)).await;
                continue;
            }

            if !resp.status().is_success() {
                return Err(format!(
                    "Discord API error: {} - {}",
                    resp.status(),
                    resp.text().await.unwrap_or_default()
                )
                .into());
            }

            break resp.json().await?;
        };

        if guilds.is_empty() {
            break;
        }

        let page_len = guilds.len();
        let last_id = guilds
            .last()
            .map(|g| g.id.clone())
            .expect("non-empty page checked above");
        all_guilds.extend(guilds.into_iter().map(|g| g.id));

        // 페이지가 limit보다 작으면 마지막 페이지.
        if page_len < 200 {
            break;
        }
        after = Some(last_id);
    }

    Ok(all_guilds)
}
