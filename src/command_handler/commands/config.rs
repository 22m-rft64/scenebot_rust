use serenity::{
    async_trait,
    builder::{
        CreateActionRow, CreateButton, CreateCommand, CreateEmbed, CreateInteractionResponse,
        CreateInteractionResponseMessage, EditInteractionResponse,
    },
    client::Context,
    futures::StreamExt,
    model::{
        application::{ButtonStyle, CommandDataOption, CommandInteraction},
        permissions::Permissions,
        prelude::Message,
    },
    Error,
};

use mongodb::{bson::doc, Collection};

use bson::Document;

use crate::command_handler::explicit_command_list::CommandInterface;
use crate::utils::{guild_config::GuildConfig, scene_core::ImageSize};
use crate::{DatabaseClient, GlobalGuildConfigs};

use log::{error, info};

use std::{num::NonZeroU64, time::Duration};

struct GuildConfigSetting;

pub fn get_command() -> Box<dyn CommandInterface + Sync + Send> {
    Box::new(GuildConfigSetting)
}

#[async_trait]
impl CommandInterface for GuildConfigSetting {
    async fn run(
        &self,
        ctx: &Context,
        _options: &[CommandDataOption],
        command: &CommandInteraction,
    ) -> Result<Message, Error> {
        let Some(guild_id) = command.guild_id else {
            return command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::default()
                        .content("이 명령어는 서버에서만 사용할 수 있습니다."),
                )
                .await;
        };
        let Some(guild_key) = NonZeroU64::new(guild_id.get()) else {
            return command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::default()
                        .content("설정 정보를 가져오는데 실패했습니다."),
                )
                .await;
        };

        let counter_lock = {
            let data_read = ctx.data.read().await;
            match data_read.get::<GlobalGuildConfigs>() {
                Some(c) => c.clone(),
                None => {
                    error!("GlobalGuildConfigs not initialised");
                    return command
                        .edit_response(
                            &ctx.http,
                            EditInteractionResponse::default()
                                .content("설정 정보를 가져오는데 실패했습니다."),
                        )
                        .await;
                }
            }
        };
        let db_client = {
            let data_read = ctx.data.read().await;
            match data_read.get::<DatabaseClient>() {
                Some(c) => c.clone(),
                None => {
                    error!("DatabaseClient not initialised");
                    return command
                        .edit_response(
                            &ctx.http,
                            EditInteractionResponse::default()
                                .content("DB 클라이언트를 가져오지 못했습니다."),
                        )
                        .await;
                }
            }
        };

        let command = command.clone();
        let mut guilds_config = counter_lock.write().await;
        let guild_config = guilds_config.get_mut(&guild_key);

        match guild_config {
            None => {
                command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::default()
                            .content("설정 정보를 가져오는데 실패했습니다."),
                    )
                    .await
            }
            Some(gc) => {
                let mut gclock = gc.lock().await;
                if let Err(why) = command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::default()
                            .add_embed(config_embed(
                                gclock.auto_magnitute_enable,
                                gclock.auto_magnitute_config.clone(),
                            ))
                            .components(vec![config_components()]),
                    )
                    .await
                {
                    error!("Failed to response slash command: {:#?}", why);
                };

                let msg = match command.get_response(&ctx.http).await {
                    Ok(m) => m,
                    Err(why) => {
                        error!("Couldn't get message info from interaction.\n{:#?}", why);
                        return Err(why);
                    }
                };

                let mut interaction_stream = msg
                    .clone()
                    .await_component_interactions(ctx)
                    .timeout(Duration::from_secs(60 * 5))
                    .filter({
                        let cmd_user = command.user.id;
                        let msg_id = msg.id;
                        move |f| {
                            f.message.id == msg_id
                                && f.member
                                    .as_ref()
                                    .map(|m| m.user.id == cmd_user)
                                    .unwrap_or(false)
                        }
                    })
                    .stream();

                if let Some(button_reaction) = interaction_stream.next().await {
                    if let Err(why) = button_reaction
                        .create_response(
                            &ctx.http,
                            CreateInteractionResponse::UpdateMessage(config_complete_component(
                                button_reaction.data.custom_id.as_str(),
                                &mut gclock,
                            )),
                        )
                        .await
                    {
                        error!("[button_reaction] sending error: {:#?}", why);
                    }

                    if button_reaction.data.custom_id == "set_default_autoemoji_size" {
                        let inner_msg = match button_reaction.get_response(&ctx.http).await {
                            Ok(m) => m,
                            Err(why) => {
                                error!("get_response failed: {:?}", why);
                                return Err(why);
                            }
                        };
                        let mut inner_stream = inner_msg
                            .clone()
                            .await_component_interactions(ctx)
                            .timeout(Duration::from_secs(60 * 5))
                            .filter({
                                let cmd_user = command.user.id;
                                let im_id = inner_msg.id;
                                move |f| {
                                    f.message.id == im_id
                                        && f.member
                                            .as_ref()
                                            .map(|m| m.user.id == cmd_user)
                                            .unwrap_or(false)
                                }
                            })
                            .stream();

                        if let Some(sizebutton_reaction) = inner_stream.next().await {
                            let size = match sizebutton_reaction.data.custom_id.as_str() {
                                "setemoji_smallest" => ImageSize::HyperTechniqueOfLisaSuFinger,
                                "setemoji_small" => ImageSize::Small,
                                "setemoji_medium" => ImageSize::Medium,
                                "setemoji_large" => ImageSize::Large,
                                "setemoji_largest" => {
                                    ImageSize::HyperSuperUltraSexFeaturedFuckingLarge
                                }
                                _ => ImageSize::Auto,
                            };

                            gclock.auto_magnitute_config = size;

                            if let Err(why) = sizebutton_reaction
                                .create_response(
                                    &ctx.http,
                                    CreateInteractionResponse::UpdateMessage(
                                        CreateInteractionResponseMessage::new()
                                            .content(format!(
                                                "자동 이모지 변환 사이즈를 {}(으)로 설정했습니다.",
                                                match sizebutton_reaction
                                                    .data
                                                    .custom_id
                                                    .as_str()
                                                {
                                                    "setemoji_smallest" => "절라 짝게",
                                                    "setemoji_small" => "작게",
                                                    "setemoji_medium" => "중간",
                                                    "setemoji_large" => "크게",
                                                    "setemoji_largest" => "존,나 크게",
                                                    _ => "자동",
                                                }
                                            ))
                                            .components(vec![])
                                            .embeds(vec![]),
                                    ),
                                )
                                .await
                            {
                                error!("sending error: {:?}", why);
                            }
                        }
                    }
                }

                // 메모리상 변경 사항 → DB persist. 공유 DB client 재사용.
                let db_name = match std::env::var("BOT_DB_NAME") {
                    Ok(v) => v,
                    Err(e) => {
                        error!("BOT_DB_NAME not set: {:?}", e);
                        return Ok(msg);
                    }
                };
                let collections: Collection<Document> =
                    db_client.database("scene").collection(&db_name);
                let gid_i64 = gclock.guild_id.get() as i64;
                if let Err(why) = collections
                    .find_one_and_update(
                        doc! { "guild_id": gid_i64 },
                        doc! {
                            "$set": {
                                "guild_id": gid_i64,
                                "auto_magnitute_enable": gclock.auto_magnitute_enable,
                                "auto_magnitute_config": ImageSize::value_to_string(
                                    &gclock.auto_magnitute_config,
                                ),
                            }
                        },
                        None,
                    )
                    .await
                {
                    error!("DB update failed for guild {}: {:?}", gid_i64, why);
                }

                info!("updated config of guild\n{:#?}", gclock);

                Ok(msg)
            }
        }
    }

    fn name(&self) -> String {
        String::from("config")
    }

    fn register(&self) -> CreateCommand {
        CreateCommand::new(self.name())
            .description("이 서버의 봇 설정을 편집해요")
            .default_member_permissions(Permissions::ADMINISTRATOR)
    }
}

fn config_embed(autoemoji: bool, default_emoji_size: ImageSize) -> CreateEmbed {
    CreateEmbed::default()
        .title("봇 설정")
        .description("설정하고 싶은 것을 선택해주세요")
        .fields([
            (
                "자동 이모지 크기 조절 설정 : 켜져있으면 끄고, 꺼져있으면 킵니다.",
                match autoemoji {
                    true => "현재 상태 : 켜짐",
                    false => "현재 상태 : 꺼짐",
                },
                false,
            ),
            (
                "이모지 크기 조정 기본값",
                match default_emoji_size {
                    ImageSize::HyperTechniqueOfLisaSuFinger => "절라 짝음",
                    ImageSize::Small => "작음",
                    ImageSize::Medium => "적당함",
                    ImageSize::Large => "큼",
                    ImageSize::HyperSuperUltraSexFeaturedFuckingLarge => "절라 큼",
                    ImageSize::Auto => "자동",
                },
                false,
            ),
        ])
        .color((255, 255, 255))
}

fn config_components() -> CreateActionRow {
    CreateActionRow::Buttons(vec![
        CreateButton::new("autoemoji_enabled")
            .label("자동 이모지 크기 조절 켜거나 끄기")
            .style(ButtonStyle::Primary),
        CreateButton::new("set_default_autoemoji_size")
            .label("크기 기본값 설정하기")
            .style(ButtonStyle::Secondary),
    ])
}

fn size_notice() -> String {
    "**__설정하고싶은 크기를 선택해주세요.__**\n\n\
순서대로 \"**절라 짝게**\", \"**작게**\", \"**중간**\", \"**큼**\", \"**절라 큼**\" 입니다.\n\
\"**작게**\"는 일반 이모지 사이즈, \"**중간**\"은 일반 스티커 사이즈입니다.\n\
\"자동\"은 이모지의 원래 크기에 따라 자동으로 바꿔집니다.\n\
즉, 이미지 크기 변환 과정이 없어 전송 속도가 가장 빠릅니다."
        .to_string()
}

fn size_component() -> Vec<CreateActionRow> {
    vec![
        CreateActionRow::Buttons(vec![
            CreateButton::new("setemoji_smallest")
                .label("절라 짝게")
                .style(ButtonStyle::Danger),
            CreateButton::new("setemoji_small")
                .label("작게")
                .style(ButtonStyle::Secondary),
            CreateButton::new("setemoji_medium")
                .label("보통")
                .style(ButtonStyle::Secondary),
            CreateButton::new("setemoji_large")
                .label("크게")
                .style(ButtonStyle::Secondary),
            CreateButton::new("setemoji_largest")
                .label("개크게")
                .style(ButtonStyle::Danger),
        ]),
        CreateActionRow::Buttons(vec![CreateButton::new("setemoji_auto")
            .label("자동")
            .style(ButtonStyle::Primary)]),
    ]
}

fn config_complete_component(
    custom_id: &str,
    gclock: &mut GuildConfig,
) -> CreateInteractionResponseMessage {
    match custom_id {
        "autoemoji_enabled" => CreateInteractionResponseMessage::default()
            .content(match gclock.auto_magnitute_enable {
                false => {
                    gclock.auto_magnitute_enable = true;
                    "자동 이모지 크기 조절이 켜졌습니다.\n이제 이모지를 전송하면 설정해둔 크기에 맞게 자동으로 봇이 변환해줍니다."
                }
                true => {
                    gclock.auto_magnitute_enable = false;
                    "자동 이모지 크기 조절이 꺼졌습니다."
                }
            })
            .components(vec![])
            .embeds(vec![]),
        _ => CreateInteractionResponseMessage::default()
            .content(size_notice())
            .components(size_component()),
    }
}
