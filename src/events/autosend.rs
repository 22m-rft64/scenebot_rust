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

use crate::utils::scene_core::{get_resized_image, merge_multiple_images, EmojiFilter, Layout};
use crate::utils::webhook::send_as_user;
use std::num::NonZeroU64;

lazy_static! {
    static ref SINGLE_EMOJI_RE: Regex = Regex::new(r"^<a?:.+?:\d+>$").unwrap();
    static ref EMOJI_TOKEN_RE: Regex = Regex::new(r"<a?:.+?:\d+>").unwrap();
    static ref EMOJI_LINE_RE: Regex = Regex::new(r"^(<a?:.+?:\d+>)+$").unwrap();
}

fn parse_emoji_line(line: &str) -> Option<Vec<&str>> {
    if !EMOJI_LINE_RE.is_match(line) {
        return None;
    }
    let tokens: Vec<&str> = EMOJI_TOKEN_RE
        .find_iter(line)
        .map(|m| m.as_str())
        .collect();
    if tokens.is_empty() {
        None
    } else {
        Some(tokens)
    }
}

fn emoji_id(token: &str) -> Option<&str> {
    // token: <a?:name:ID>
    token.rsplit(':').next().and_then(|s| s.strip_suffix('>'))
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

    fn multi_emoji_format_filter(&self) -> Result<(Layout, Vec<String>), ()> {
        if self.author.bot
            || !self.attachments.is_empty()
            || !self.mentions.is_empty()
            || self.referenced_message.is_some()
        {
            return Err(());
        }

        // `\r\n`까지 처리하고 각 줄 양끝 공백 trim. 빈 줄은 무시.
        let lines: Vec<&str> = self
            .content
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        if lines.is_empty() {
            return Err(());
        }

        // 각 라인이 이모지 토큰만으로 구성되어야 함.
        let mut parsed: Vec<Vec<&str>> = Vec::with_capacity(lines.len());
        for line in &lines {
            match parse_emoji_line(line) {
                Some(tokens) => parsed.push(tokens),
                None => {
                    log::debug!("multi_emoji_filter: line not all-emoji: {:?}", line);
                    return Err(());
                }
            }
        }

        // GIF(animated) 이모지가 하나라도 포함되면 multi-emoji merge 비활성.
        if parsed.iter().flatten().any(|t| t.starts_with("<a:")) {
            return Err(());
        }

        let counts: Vec<usize> = parsed.iter().map(|l| l.len()).collect();
        let total: usize = counts.iter().sum();
        log::debug!(
            "multi_emoji_filter: lines={}, counts={:?}",
            parsed.len(),
            counts
        );

        let layout = if parsed.len() == 1 && (2..=4).contains(&counts[0]) {
            Layout::Horizontal
        } else if (2..=3).contains(&parsed.len()) && counts.iter().all(|&c| c == 1) {
            Layout::Vertical
        } else if parsed.len() == 2 && counts.iter().all(|&c| c == 2) {
            Layout::Grid2x2
        } else if parsed.len() == 3 && counts.iter().all(|&c| c == 3) {
            Layout::Grid3x3
        } else {
            log::debug!("multi_emoji_filter: no layout match");
            return Err(());
        };

        let urls: Vec<String> = parsed
            .into_iter()
            .flatten()
            .filter_map(|token| {
                emoji_id(token)
                    .map(|id| format!("https://cdn.discordapp.com/emojis/{}.png?size=128", id))
            })
            .collect();
        if urls.len() != total {
            return Err(());
        }

        Ok((layout, urls))
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

pub async fn auto_send_multi_emoji(ctx: &Context, msg: &Message) {
    let (layout, urls) = match msg.multi_emoji_format_filter() {
        Ok(r) => r,
        Err(()) => return,
    };

    let guild_id = match msg.guild_id {
        Some(id) => match NonZeroU64::new(id.get()) {
            Some(id) => id,
            None => return,
        },
        None => return,
    };

    if get_guild_config(ctx, guild_id).await.is_none() {
        return;
    }

    if let Err(why) = msg.delete(&ctx.http).await {
        error!("couldn't delete message, skipping resend: {:?}", why);
        return;
    }

    let merged = match merge_multiple_images(&urls, layout).await {
        Ok(img) => img,
        Err(e) => {
            error!("Failed to merge ({:?}): {:?}", layout, e);
            return;
        }
    };

    if let Err(why) = send_merged_emoji(ctx, msg, merged).await {
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
) -> Result<(), serenity::Error> {
    if send_as_user(ctx, msg, Some(format!("{}{}", img_url, size)), vec![])
        .await
        .is_ok()
    {
        return Ok(());
    }

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
        .map(|_| ())
}

async fn send_emoji_as_file(
    ctx: &Context,
    msg: &Message,
    is_png: bool,
    img_url: &str,
    size_config: &ImageSize,
) -> Result<(), serenity::Error> {
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

    if send_as_user(ctx, msg, None, vec![attachment.clone()])
        .await
        .is_ok()
    {
        return Ok(());
    }

    let display_name = get_user_display_name(msg);
    msg.channel_id
        .send_files(
            &ctx.http,
            [attachment],
            CreateMessage::new().content(format!("**{}** :", display_name)),
        )
        .await
        .map(|_| ())
}

async fn send_merged_emoji(
    ctx: &Context,
    msg: &Message,
    merged_image: Vec<u8>,
) -> Result<(), serenity::Error> {
    let attachment = CreateAttachment::bytes(merged_image, "merged_emoji.png");

    if send_as_user(ctx, msg, None, vec![attachment.clone()])
        .await
        .is_ok()
    {
        return Ok(());
    }

    let display_name = get_user_display_name(msg);
    msg.channel_id
        .send_files(
            &ctx.http,
            vec![attachment],
            CreateMessage::new().content(format!("**{}** :", display_name)),
        )
        .await
        .map(|_| ())
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
