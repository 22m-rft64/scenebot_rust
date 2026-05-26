use serenity::{
    all::UnavailableGuild,
    async_trait,
    client::{Context, EventHandler},
    gateway::ActivityData,
    model::{application::Interaction, channel::Message, gateway::Ready, guild::Guild},
};

use crate::command_handler::handler::seperate_command;
use crate::utils::guild_config::GuildConfig;

use log::info;

pub struct DiscordEventHandler {
    pub database: mongodb::Client,
}

use crate::events::{autosend, guild_delete, guildbotadd};

#[async_trait]
impl EventHandler for DiscordEventHandler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{}으로 로그인 완료!", ready.user.tag());

        ctx.set_activity(Some(ActivityData::playing(format!(
            "이모지 확대용 봇 | {}개의 서버에서 일하는중",
            ctx.cache.guilds().len()
        ))));

        tokio::join!(GuildConfig::boot(&ctx, &(self.database)));
    }

    async fn guild_create(&self, ctx: Context, guild: Guild, is_new: Option<bool>) {
        tokio::join!(
            guildbotadd::senddm(&ctx, &guild, is_new),
            guildbotadd::set_bot_status(&ctx),
            guildbotadd::new_guild_added(&ctx, &guild, is_new),
            guildbotadd::db_fetch(&ctx, &guild, is_new, &(self.database))
        );
    }

    async fn guild_delete(&self, ctx: Context, incomplete: UnavailableGuild, full: Option<Guild>) {
        tokio::join!(guild_delete::db_delete(
            &ctx,
            &incomplete,
            &full,
            &(self.database)
        ));
    }

    async fn message(&self, ctx: Context, msg: Message) {
        tokio::join!(
            autosend::auto_send_transfered_image(&ctx, &msg),
            autosend::auto_send_multi_emoji(&ctx, &msg)
        );
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            seperate_command(command, &ctx).await;
        }
    }
}
