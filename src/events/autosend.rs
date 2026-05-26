use lazy_static::lazy_static;
use log::error;
use regex::Regex;
use serenity::{
    builder::{CreateAttachment, CreateMessage},
    client::Context,
    model::channel::Message,
};

use crate::utils::scene_core::ImageSize::{
    self, Auto, HyperTechniqueOfLisaSuFinger, Medium, Small,
};
use crate::GlobalGuildConfigs;

use crate::utils::scene_core::{get_resized_image, merge_two_emojis, EmojiFilter};
use std::num::NonZeroU64;

lazy_static! {
    static ref SINGLE_EMOJI_RE: Regex = Regex::new(r"^<a?:.+?:\d+>$").unwrap();
    static ref DOUBLE_EMOJI_RE: Regex = Regex::new(r"^(<a?:.+?:\d+>)(<a?:.+?:\d+>)$").unwrap();
}

impl EmojiFilter for Message {
    fn emoji_format_filter(&self) -> Result<(bool, String), ()> {
        let msg_content_vec: Vec<&str> = self.content.split(':').collect();

        if self.referenced_message.is_some()
            || self.author.bot
            || !self.attachments.is_empty()
            || !SINGLE_EMOJI_RE.is_match(&self.content)
            || !self.mentions.is_empty()
            || msg_content_vec.len() != 3
        {
            return Err(());
        }

        let mut id = msg_content_vec[2].to_string();
        id.pop();
        let is_png = !self.content.contains("<a:");

        let img_url = format!(
            "https://cdn.discordapp.com/emojis/{}.{}",
            id,
            if is_png { "webp" } else { "gif" }
        );

        Ok((is_png, img_url))
    }

    fn double_emoji_format_filter(&self) -> Result<(bool, String, String), ()> {
        if !DOUBLE_EMOJI_RE.is_match(&self.content)
            || self.author.bot
            || !self.attachments.is_empty()
            || !self.mentions.is_empty()
            || self.referenced_message.is_some()
        {
            return Err(());
        }

        let captures = match DOUBLE_EMOJI_RE.captures(&self.content) {
            Some(c) => c,
            None => return Err(()),
        };
        let first_emoji = match captures.get(1) {
            Some(m) => m.as_str(),
            None => return Err(()),
        };
        let second_emoji = match captures.get(2) {
            Some(m) => m.as_str(),
            None => return Err(()),
        };

        let first_is_animated = first_emoji.starts_with("<a:");
        let second_is_animated = second_emoji.starts_with("<a:");

        if first_is_animated != second_is_animated {
            return Err(());
        }

        let first_id = {
            let parts: Vec<&str> = first_emoji.split(':').collect();
            if parts.len() != 3 {
                return Err(());
            }
            let mut id = parts[2].to_string();
            id.pop();
            id
        };

        let second_id = {
            let parts: Vec<&str> = second_emoji.split(':').collect();
            if parts.len() != 3 {
                return Err(());
            }
            let mut id = parts[2].to_string();
            id.pop();
            id
        };

        let is_png = !first_is_animated;
        let format = if is_png { "webp" } else { "gif" };

        let first_url = format!("https://cdn.discordapp.com/emojis/{}.{}", first_id, format);
        let second_url = format!("https://cdn.discordapp.com/emojis/{}.{}", second_id, format);

        Ok((is_png, first_url, second_url))
    }
}

pub async fn auto_send_transfered_image(ctx: &Context, msg: &Message) {
    let filtered = match msg.emoji_format_filter() {
        Ok(result) => result,
        Err(()) => return,
    };

    let guild_id = match msg.guild_id {
        Some(id) => match NonZeroU64::new(id.get()) {
            Some(id) => id,
            None => return,
        },
        None => return,
    };

    let config = match get_guild_config(ctx, guild_id).await {
        Some(config) => config,
        None => return,
    };

    let (_, size_config) = config;

    // 메시지 삭제 실패하면 중복 전송을 막기 위해 그냥 종료.
    if let Err(why) = msg.delete(&ctx.http).await {
        error!("couldn't delete message, skipping resend: {:?}", why);
        return;
    }

    let (is_png, img_url) = filtered;

    let result = if matches!(
        size_config,
        HyperTechniqueOfLisaSuFinger | Small | Medium | Auto
    ) {
        let size = match size_config {
            HyperTechniqueOfLisaSuFinger => "?size=16",
            Small => "?size=64",
            Medium => "?size=128",
            _ => "",
        };

        send_emoji_as_url(ctx, msg, &img_url, size).await
    } else {
        send_emoji_as_file(ctx, msg, is_png, &img_url, &size_config).await
    };

    if let Err(why) = result {
        error!("send message error: {:?}", why);
    }
}

pub async fn auto_send_double_emoji(ctx: &Context, msg: &Message) {
    let filtered = match msg.double_emoji_format_filter() {
        Ok(result) => result,
        Err(()) => return,
    };

    // 현재는 정적 이모지(PNG/WebP) 더블콘만 지원. GIF는 아직 미구현.
    let (is_png, first_url, second_url) = filtered;
    if !is_png {
        return;
    }

    let guild_id = match msg.guild_id {
        Some(id) => match NonZeroU64::new(id.get()) {
            Some(id) => id,
            None => return,
        },
        None => return,
    };

    let Some(_config) = get_guild_config(ctx, guild_id).await else {
        return;
    };

    if let Err(why) = msg.delete(&ctx.http).await {
        error!("couldn't delete message, skipping resend: {:?}", why);
        return;
    }

    let result = match merge_two_emojis(&first_url, &second_url).await {
        Ok(merged_image) => send_merged_emoji(ctx, msg, merged_image).await,
        Err(e) => {
            error!("Failed to merge emojis: {:?}", e);
            return;
        }
    };

    if let Err(why) = result {
        error!("send message error: {:?}", why);
    }
}

async fn get_guild_config(ctx: &Context, guild_id: NonZeroU64) -> Option<(bool, ImageSize)> {
    let counter_lock = {
        let data_read = ctx.data.read().await;
        data_read.get::<GlobalGuildConfigs>()?.clone()
    };

    let guilds_config = counter_lock.read().await;
    let gconfig_lock = guilds_config.get(&guild_id)?;

    let gconfig = gconfig_lock.lock().await;

    if !gconfig.auto_magnitute_enable {
        return None;
    }

    Some((
        gconfig.auto_magnitute_enable,
        gconfig.auto_magnitute_config.clone(),
    ))
}

async fn send_emoji_as_url(
    ctx: &Context,
    msg: &Message,
    img_url: &str,
    size: &str,
) -> Result<Message, serenity::Error> {
    let display_name = get_user_display_name(msg);

    if let Err(why) = msg
        .channel_id
        .say(&ctx.http, format!("**{}** :", display_name))
        .await
    {
        error!("couldn't send display-name line: {:?}", why);
    }

    msg.channel_id
        .say(&ctx.http, format!("{}{}", img_url, size))
        .await
}

async fn send_emoji_as_file(
    ctx: &Context,
    msg: &Message,
    is_png: bool,
    img_url: &str,
    size_config: &ImageSize,
) -> Result<Message, serenity::Error> {
    let display_name = get_user_display_name(msg);

    let attachment = if is_png {
        match get_resized_image(ctx, img_url, size_config).await {
            Some(att) => att,
            None => {
                error!("resize failed, falling back to direct URL attachment");
                CreateAttachment::url(&ctx.http, img_url).await?
            }
        }
    } else {
        CreateAttachment::url(&ctx.http, img_url).await?
    };

    msg.channel_id
        .send_files(
            &ctx.http,
            [attachment],
            CreateMessage::new().content(format!("**{}** :", display_name)),
        )
        .await
}

async fn send_merged_emoji(
    ctx: &Context,
    msg: &Message,
    merged_image: Vec<u8>,
) -> Result<Message, serenity::Error> {
    let display_name = get_user_display_name(msg);

    let attachment = CreateAttachment::bytes(merged_image, "double_emoji.png");

    msg.channel_id
        .send_files(
            &ctx.http,
            vec![attachment],
            CreateMessage::new().content(format!("**{}** :", display_name)),
        )
        .await
}

fn get_user_display_name(msg: &Message) -> String {
    let global_username = msg
        .author
        .global_name
        .clone()
        .unwrap_or_else(|| msg.author.name.clone());

    match &msg.member {
        Some(m) => match &m.nick {
            Some(nick) => nick.clone(),
            None => global_username,
        },
        None => global_username,
    }
}
