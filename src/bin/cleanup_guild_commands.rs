// 일회성 정리 스크립트:
//   예전엔 명령어를 서버별로 등록했음 (/send, /help, /update 등).
//   글로벌 명령어로 바꾸면서 남아있는 서버별 명령어를 전부 지움.
//
// 사용:
//   cd scenebot_rust
//   .env 준비 (DISCORD_BOT_TOKEN)
//   cargo run --release --bin cleanup_guild_commands
//
// 멱등: 이미 비어있는 서버는 그냥 빈 목록으로 다시 덮어씀.

use dotenvy::dotenv;
use env_logger::init;
use log::{error, info};
use serenity::{http::Http, model::guild::GuildInfo, model::id::GuildId};
use serenity::all::GuildPagination;
use std::{env, error::Error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    init();

    let token = env::var("DISCORD_BOT_TOKEN").expect("DISCORD_BOT_TOKEN is not set");
    let http = Http::new(&token);
    let app = http.get_current_application_info().await?;
    http.set_application_id(app.id);

    let mut guilds: Vec<GuildInfo> = Vec::new();
    loop {
        let after = guilds.last().map(|g| GuildPagination::After(g.id));
        let page = http.get_guilds(after, Some(200)).await?;
        let done = page.len() < 200;
        guilds.extend(page);
        if done {
            break;
        }
    }
    info!("{} guilds", guilds.len());

    let mut failed = 0;
    for g in &guilds {
        let gid: GuildId = g.id;
        match gid.set_commands(&http, vec![]).await {
            Ok(_) => info!("cleared: {} ({})", g.name, gid),
            Err(why) => {
                failed += 1;
                error!("failed: {} ({}): {:?}", g.name, gid, why);
            }
        }
    }

    info!("done. {} guilds, {} failed", guilds.len(), failed);
    Ok(())
}
