use crate::{utils::scene_core::ImageSize, GlobalGuildConfigs};
use log::{error, info, warn};

use mongodb::bson::{doc, Bson};
use serde::{Deserialize, Serialize};
use serenity::{
    all::Permissions, builder::CreateCommand, client::Context, gateway::ActivityData,
    model::id::GuildId,
};
use std::num::NonZeroU64;

#[derive(Debug, Serialize, Deserialize)]
pub struct GuildConfig {
    pub guild_id: NonZeroU64,
    pub auto_magnitute_enable: bool,
    pub auto_magnitute_config: ImageSize,
}

impl GuildConfig {
    pub fn new(guild: &GuildId) -> Self {
        Self {
            guild_id: NonZeroU64::new(guild.get()).expect("guild id cannot be zero"),
            auto_magnitute_enable: false,
            auto_magnitute_config: ImageSize::Auto,
        }
    }

    pub fn load(
        guild: &GuildId,
        auto_magnitute_enable_input: bool,
        auto_magnitute_config_input: ImageSize,
    ) -> Self {
        Self {
            guild_id: NonZeroU64::new(guild.get()).expect("guild id cannot be zero"),
            auto_magnitute_enable: auto_magnitute_enable_input,
            auto_magnitute_config: auto_magnitute_config_input,
        }
    }

    pub async fn delete(guild: &GuildId, ctx: &Context) -> Result<(), ()> {
        let counter_lock = {
            let data_read = ctx.data.read().await;
            match data_read.get::<GlobalGuildConfigs>() {
                Some(c) => c.clone(),
                None => {
                    error!("GlobalGuildConfigs not initialised");
                    return Err(());
                }
            }
        };
        let mut guilds_config = counter_lock.write().await;
        match NonZeroU64::new(guild.get()) {
            Some(key) => match guilds_config.remove(&key) {
                Some(_) => Ok(()),
                None => Err(()),
            },
            None => Err(()),
        }
    }

    pub async fn boot(ctx: &Context, database: &mongodb::Client) {
        info!(
            "loaded {} guilds. fetching config data from DB..",
            ctx.cache.guilds().len()
        );

        let db_name = match std::env::var("BOT_DB_NAME") {
            Ok(v) => v,
            Err(e) => {
                error!("BOT_DB_NAME not set: {:?}", e);
                return;
            }
        };
        let collections: mongodb::Collection<bson::document::Document> =
            database.database("scene").collection(&db_name);

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
        ctx.set_activity(Some(ActivityData::playing("부팅")));

        for guild in ctx.cache.guilds() {
            let sync_collections = collections.clone();
            let sync_guilds_config = counter_lock.clone();
            if let Err(why) = guild
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
                    guild, why
                );
            }
            tokio::spawn(async move {
                let Some(key) = NonZeroU64::new(guild.get()) else {
                    error!("guild id is zero: {:?}", guild);
                    return;
                };
                info!("loading guild {}..", key);

                let gid_i64 = guild.get() as i64;
                let gid_f64 = guild.get() as f64;

                // 1) Try the new i64 storage first.
                let find_result = sync_collections
                    .find_one(doc! { "guild_id": gid_i64 }, None)
                    .await;

                let found_doc = match find_result {
                    Ok(opt) => opt,
                    Err(why) => {
                        error!(
                            "an error occured when loading data from DB (i64): guildid: {}\n{:?}",
                            guild.get(),
                            why
                        );
                        return;
                    }
                };

                let document = match found_doc {
                    Some(doc) => doc,
                    None => {
                        // 2) Fall back to the legacy f64 record (precision-lossy).
                        match sync_collections
                            .find_one(doc! { "guild_id": gid_f64 }, None)
                            .await
                        {
                            Ok(Some(legacy)) => {
                                // Reconcile: write an i64 record using the *real* guild id, drop the f64 doc.
                                let auto_enable =
                                    legacy.get_bool("auto_magnitute_enable").unwrap_or(false);
                                let auto_config = legacy
                                    .get_str("auto_magnitute_config")
                                    .unwrap_or("Auto")
                                    .to_string();
                                if let Err(why) = sync_collections
                                    .delete_one(doc! { "guild_id": gid_f64 }, None)
                                    .await
                                {
                                    warn!(
                                        "legacy f64 doc delete failed (guildid {}): {:?}",
                                        guild.get(),
                                        why
                                    );
                                }
                                if let Err(why) = sync_collections
                                    .insert_one(
                                        doc! {
                                            "guild_id": gid_i64,
                                            "auto_magnitute_enable": auto_enable,
                                            "auto_magnitute_config": auto_config.clone(),
                                        },
                                        None,
                                    )
                                    .await
                                {
                                    error!(
                                        "reconcile insert failed (guildid {}): {:?}",
                                        guild.get(),
                                        why
                                    );
                                    return;
                                }
                                info!(
                                    "guild {} reconciled from f64 → i64 (auto_enable={}, config={})",
                                    guild.get(),
                                    auto_enable,
                                    auto_config
                                );
                                let mut tmp = bson::Document::new();
                                tmp.insert("auto_magnitute_enable", Bson::Boolean(auto_enable));
                                tmp.insert("auto_magnitute_config", Bson::String(auto_config));
                                tmp
                            }
                            Ok(None) => {
                                // No record at all → create a fresh one.
                                if let Err(why) = sync_collections
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
                                        "Couldn't add new DB record: guildid: {}, {:?}",
                                        guild.get(),
                                        why
                                    );
                                    return;
                                }
                                let mut guilds_config = sync_guilds_config.write().await;
                                guilds_config.insert(
                                    key,
                                    std::sync::Arc::new(tokio::sync::Mutex::new(
                                        GuildConfig::new(&guild),
                                    )),
                                );
                                info!("guild {} loaded (new).", guild.get());
                                return;
                            }
                            Err(why) => {
                                error!(
                                    "an error occured when loading data from DB (f64 fallback): guildid: {}\n{:?}",
                                    guild.get(),
                                    why
                                );
                                return;
                            }
                        }
                    }
                };

                let new_config = GuildConfig::load(
                    &guild,
                    document.get_bool("auto_magnitute_enable").unwrap_or(false),
                    ImageSize::string_to_value(
                        document.get_str("auto_magnitute_config").unwrap_or("Auto"),
                    ),
                );

                let mut guilds_config = sync_guilds_config.write().await;
                guilds_config.insert(
                    key,
                    std::sync::Arc::new(tokio::sync::Mutex::new(new_config)),
                );
                info!("guild {} loaded.", guild.get());
            });
        }

        ctx.set_activity(Some(ActivityData::playing(format!(
            "이모지 확대용 봇 | {}개의 서버에서 일하는중",
            ctx.cache.guilds().len()
        ))));

        info!("booting complete.");
    }
}
