use serenity::{
    builder::CreateCommand,
    client::Context,
    gateway::ActivityData,
    model::{guild::Guild, id::UserId, prelude::Permissions},
};

use log::{error, info};
use mongodb::bson::doc;

use std::num::NonZeroU64;

use crate::utils::guild_config::GuildConfig;
use crate::GlobalGuildConfigs;

pub async fn senddm(ctx: &Context, guild: &Guild, is_new: Option<bool>) {
    if is_new.is_none() || !is_new.unwrap() {
        return;
    }
    let teddypicker = UserId::new(653157614452211712);
    match teddypicker.create_dm_channel(&ctx.http).await {
        Ok(channel) => {
            if let Err(why) = channel
                .say(
                    &ctx.http,
                    format!(
                        "봇이 **{}**에 추가됨, 서버 수 : {}개",
                        guild.name,
                        ctx.cache.guilds().len()
                    ),
                )
                .await
            {
                error!("error occured while sending dm to teddypicker: {:?}", why);
            }
        }
        Err(why) => error!("error occured while creating dm channel: {:?}", why),
    }
}

pub async fn new_guild_added(ctx: &Context, guild: &Guild, is_new: Option<bool>) {
    if is_new.is_none() || !is_new.unwrap() {
        return;
    }

    info!("new guild added: {}, ID: {}", guild.name, guild.id);

    if let Err(why) = guild
        .id
        .create_command(
            &ctx.http,
            CreateCommand::new("update")
                .description("봇의 업데이트를 확인해요")
                .default_member_permissions(Permissions::ADMINISTRATOR),
        )
        .await
    {
        error!(
            "error occured while creating update command at {}\n{:#?}",
            guild.id, why
        );
    }
}

// 봇이 새 서버에 추가되었을 때 호출됨.
// i64로 저장된 기존 doc이 있으면 그대로 사용 (재초대 시 사용자 설정 보존).
// 없으면 새 doc 생성. f64 legacy doc은 boot 시 reconcile됨.
pub async fn db_fetch(
    ctx: &Context,
    guild: &Guild,
    is_new: Option<bool>,
    database: &mongodb::Client,
) {
    if is_new.is_none() || !is_new.unwrap() {
        return;
    }

    let db_name = match std::env::var("BOT_DB_NAME") {
        Ok(v) => v,
        Err(e) => {
            error!("BOT_DB_NAME not set: {:?}", e);
            return;
        }
    };
    let collections: mongodb::Collection<bson::document::Document> =
        database.database("scene").collection(&db_name);
    let guildid = guild.id.get();
    let gid_i64 = guildid as i64;

    let Some(key) = NonZeroU64::new(guildid) else {
        error!("guild id is zero on guild_create: {}", guild.name);
        return;
    };

    let existed = match collections
        .find_one(doc! { "guild_id": gid_i64 }, None)
        .await
    {
        Ok(opt) => opt.is_some(),
        Err(why) => {
            error!(
                "couldn't query DB for guild_id {} on new-guild-add: {:?}",
                guildid, why
            );
            return;
        }
    };

    if !existed {
        if let Err(why) = collections
            .insert_one(
                doc! {
                    "guild_id": gid_i64,
                    "auto_magnitute_enable": false,
                    "auto_magnitute_config": "Auto",
                },
                None,
            )
            .await
        {
            error!(
                "Couldn't add new DB record (guildid: {}, name: {}): {:?}",
                guildid, guild.name, why
            );
            return;
        }
        info!(
            "new DB record added: guildid: {}, name: {}",
            guildid, guild.name
        );
    } else {
        info!(
            "existing DB record kept on re-invite: guildid: {}, name: {}",
            guildid, guild.name
        );
    }

    let counter_lock = {
        let data_read = ctx.data.read().await;
        match data_read.get::<GlobalGuildConfigs>() {
            Some(c) => c.clone(),
            None => {
                error!("GlobalGuildConfigs not initialised");
                return;
            }
        }
    };
    let mut guilds_config = counter_lock.write().await;
    guilds_config
        .entry(key)
        .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(GuildConfig::new(&guild.id))));
}

pub async fn set_bot_status(ctx: &Context) {
    ctx.set_activity(Some(ActivityData::playing(format!(
        "이모지 확대용 봇 | {}개의 서버에서 일하는중",
        ctx.cache.guilds().len()
    ))));
}
