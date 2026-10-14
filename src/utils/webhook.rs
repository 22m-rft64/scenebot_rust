// 유저 프사/닉네임으로 보이게 하려고 채널별 webhook을 만들어 두고 재사용함.
// 스레드는 webhook을 못 만드니까 부모 채널 webhook + thread_id로 보냄.
use log::error;
use serenity::{
    builder::{CreateAttachment, CreateWebhook, ExecuteWebhook},
    client::Context,
    model::{channel::Message, id::ChannelId, webhook::Webhook},
};

use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

use lazy_static::lazy_static;

const WEBHOOK_NAME: &str = "scene";

lazy_static! {
    static ref WEBHOOK_CACHE: Arc<RwLock<HashMap<ChannelId, Webhook>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

// (webhook을 붙일 채널, 스레드면 Some(스레드 id))
async fn resolve_channel(ctx: &Context, msg: &Message) -> Option<(ChannelId, Option<ChannelId>)> {
    let guild_id = msg.guild_id?;

    if let Some(guild) = ctx.cache.guild(guild_id) {
        if guild.channels.contains_key(&msg.channel_id) {
            return Some((msg.channel_id, None));
        }
        if let Some(th) = guild.threads.iter().find(|t| t.id == msg.channel_id) {
            return Some((th.parent_id?, Some(msg.channel_id)));
        }
    }

    let channel = msg.channel_id.to_channel(&ctx.http).await.ok()?.guild()?;
    if channel.thread_metadata.is_some() {
        Some((channel.parent_id?, Some(msg.channel_id)))
    } else {
        Some((msg.channel_id, None))
    }
}

async fn get_or_create_webhook(ctx: &Context, channel_id: ChannelId) -> Option<Webhook> {
    if let Some(wh) = WEBHOOK_CACHE.read().await.get(&channel_id) {
        return Some(wh.clone());
    }

    let bot_id = ctx.cache.current_user().id;
    let existing = match channel_id.webhooks(&ctx.http).await {
        Ok(list) => list.into_iter().find(|w| {
            w.token.is_some() && w.user.as_ref().map(|u| u.id) == Some(bot_id)
        }),
        Err(why) => {
            error!("couldn't list webhooks in {}: {:?}", channel_id, why);
            return None;
        }
    };

    let webhook = match existing {
        Some(w) => w,
        None => match channel_id
            .create_webhook(&ctx.http, CreateWebhook::new(WEBHOOK_NAME))
            .await
        {
            Ok(w) => w,
            Err(why) => {
                error!("couldn't create webhook in {}: {:?}", channel_id, why);
                return None;
            }
        },
    };

    WEBHOOK_CACHE
        .write()
        .await
        .insert(channel_id, webhook.clone());
    Some(webhook)
}

fn display_name(msg: &Message) -> String {
    let global_username = msg
        .author
        .global_name
        .clone()
        .unwrap_or_else(|| msg.author.name.clone());

    msg.member
        .as_ref()
        .and_then(|m| m.nick.clone())
        .unwrap_or(global_username)
}

// 서버 프사가 있으면 그거, 없으면 계정 프사
fn avatar_url(msg: &Message) -> String {
    if let (Some(guild_id), Some(hash)) = (
        msg.guild_id,
        msg.member.as_ref().and_then(|m| m.avatar.as_ref()),
    ) {
        let ext = if hash.is_animated() { "gif" } else { "png" };
        return format!(
            "https://cdn.discordapp.com/guilds/{}/users/{}/avatars/{}.{}?size=1024",
            guild_id, msg.author.id, hash, ext
        );
    }
    msg.author.face()
}

// 실패하면 Err로 돌려줘서 호출자가 기존 방식으로 보내게 함
pub async fn send_as_user(
    ctx: &Context,
    msg: &Message,
    content: Option<String>,
    files: Vec<CreateAttachment>,
) -> Result<(), ()> {
    let (channel_id, thread_id) = resolve_channel(ctx, msg).await.ok_or(())?;

    let mut builder = ExecuteWebhook::new()
        .username(display_name(msg))
        .avatar_url(avatar_url(msg))
        .add_files(files);
    if let Some(c) = content {
        builder = builder.content(c);
    }
    if let Some(th) = thread_id {
        builder = builder.in_thread(th);
    }

    for attempt in 0..2 {
        let webhook = get_or_create_webhook(ctx, channel_id).await.ok_or(())?;
        match webhook.execute(&ctx.http, false, builder.clone()).await {
            Ok(_) => return Ok(()),
            Err(why) => {
                error!("webhook execute failed (attempt {}): {:?}", attempt, why);
                // 누가 webhook 지웠을 수도 있으니 캐시 날리고 한 번 더
                WEBHOOK_CACHE.write().await.remove(&channel_id);
            }
        }
    }
    Err(())
}
