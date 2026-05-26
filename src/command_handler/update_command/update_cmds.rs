use serenity::{
    builder::{
        CreateActionRow, CreateButton, CreateEmbed, CreateInteractionResponse,
        CreateInteractionResponseMessage, EditInteractionResponse, EditMessage,
    },
    client::Context,
    futures::StreamExt,
    model::{
        application::{ButtonStyle, CommandInteraction},
        id::GuildId,
    },
};

use log::error;

use std::time::Duration;

use super::super::explicit_command_list::COMMAND_LIST;
use super::update_checker::{check_updates, UpdateStatus};

pub async fn update_command(command: CommandInteraction, ctx: &Context) {
    if let Err(why) = command.defer(&ctx.http).await {
        error!("defer failed on /update: {:?}", why);
        return;
    }

    match command.guild_id {
        Some(gid) => match check_updates(ctx, gid).await {
            UpdateStatus::FirstSetting => first_setup_msg(ctx, gid, command).await,
            UpdateStatus::LatestVersion => latest_version_msg(ctx, command).await,
            UpdateStatus::UpdateAvailable(unassigned_commands) => {
                update_available_msg(ctx, gid, unassigned_commands, command).await
            }
            UpdateStatus::FailedtoLoad => failed_notice_msg(ctx, command).await,
        },
        None => failed_notice_msg(ctx, command).await,
    }
}

async fn first_setup_msg(ctx: &Context, gid: GuildId, command: CommandInteraction) {
    if let Err(why) = command
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new()
                .embed(setup_embed())
                .components(vec![CreateActionRow::Buttons(Vec::from(
                    update_components(false),
                ))]),
        )
        .await
    {
        error!("Failed to response slash command: {:#?}", why);
    };

    let msg = match command.get_response(&ctx.http).await {
        Ok(m) => m,
        Err(why) => {
            error!("Couldn't get message info from interaction.\n{:#?}", why);
            return;
        }
    };

    let mut interaction_stream = msg
        .await_component_interactions(ctx)
        .timeout(Duration::from_secs(60 * 5))
        .filter(move |f| {
            f.message.id == msg.id
                && f.member
                    .as_ref()
                    .map(|m| m.user.id == command.user.id)
                    .unwrap_or(false)
        })
        .stream();

    if let Some(button_reaction) = interaction_stream.next().await {
        match button_reaction.data.custom_id.as_str() {
            "update_cmds" => {
                match button_reaction
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::UpdateMessage(
                            CreateInteractionResponseMessage::new()
                                .embed(
                                    CreateEmbed::new()
                                        .title("서버로부터 명령어를 등록하는 중..."),
                                )
                                .components(vec![CreateActionRow::Buttons(Vec::from(
                                    update_components(true),
                                ))]),
                        ),
                    )
                    .await
                {
                    Ok(_) => {
                        COMMAND_LIST.register_commands(gid, ctx).await;
                        if let Err(why) = button_reaction
                            .edit_response(
                                &ctx,
                                EditInteractionResponse::new().embed(
                                    CreateEmbed::new().title("명령어 등록이 완료되었습니다."),
                                ),
                            )
                            .await
                        {
                            error!("Couldn't send complete msg. {:#?}", why);
                        }
                    }
                    Err(why) => {
                        error!("Couldn't edit response msg., {:#?}", why);
                    }
                }
            }
            "show_patchnotes" => {
                if let Err(why) = button_reaction
                    .edit_response(
                        &ctx,
                        EditInteractionResponse::new()
                            .content(super::load_patchnote::get_patchnote() + "여기서 확인 ㄱㄱ")
                            .components(vec![])
                            .embeds(vec![]),
                    )
                    .await
                {
                    error!("Couldn't send complete msg. {:#?}", why);
                }
            }
            _ => {
                if let Err(why) = command.delete_response(&ctx.http).await {
                    error!("delete_response failed: {:?}", why);
                }
            }
        }
    }
}

async fn latest_version_msg(ctx: &Context, command: CommandInteraction) {
    if let Err(why) = command
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new().content("✅ 이 서버에는 업데이트 할 명령어가 없습니다."),
        )
        .await
    {
        error!("Failed to response slash command: {:#?}", why);
    };
}

async fn update_available_msg(
    ctx: &Context,
    gid: GuildId,
    unassigned_commands: Vec<String>,
    command: CommandInteraction,
) {
    if let Err(why) = command
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new()
                .embed(
                    CreateEmbed::new()
                        .title("ℹ️ 아직 등록되지 않은 명령어가 있습니다")
                        .description(unassigned_commands.join("\n")),
                )
                .components(vec![CreateActionRow::Buttons(Vec::from(
                    update_components(false),
                ))]),
        )
        .await
    {
        error!("Failed to response slash command: {:#?}", why);
    };

    let mut msg = match command.get_response(&ctx.http).await {
        Ok(m) => m,
        Err(why) => {
            error!("Couldn't get message info from interaction.\n{:#?}", why);
            return;
        }
    };

    let mut interaction_stream = msg
        .clone()
        .await_component_interactions(ctx)
        .timeout(Duration::from_secs(60 * 5))
        .filter(move |f| {
            f.message.id == msg.id
                && f.member
                    .as_ref()
                    .map(|m| m.user.id == command.user.id)
                    .unwrap_or(false)
        })
        .stream();

    if let Some(button_reaction) = interaction_stream.next().await {
        match button_reaction.data.custom_id.as_str() {
            "update_cmds" => {
                match button_reaction
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::UpdateMessage(
                            CreateInteractionResponseMessage::new()
                                .embed(
                                    CreateEmbed::new()
                                        .title("서버로부터 명령어를 등록하는 중..."),
                                )
                                .components(vec![CreateActionRow::Buttons(Vec::from(
                                    update_components(true),
                                ))]),
                        ),
                    )
                    .await
                {
                    Ok(_) => {
                        COMMAND_LIST.register_commands(gid, ctx).await;

                        if let Err(why) = msg
                            .edit(
                                &ctx.http,
                                EditMessage::new().embed(
                                    CreateEmbed::new().title("명령어 등록이 완료되었습니다."),
                                ),
                            )
                            .await
                        {
                            error!("Couldn't send complete msg. {:#?}", why);
                        }
                    }
                    Err(why) => {
                        error!("Couldn't edit response msg., {:#?}", why);
                    }
                }
            }
            "show_patchnotes" => {
                if let Err(why) = button_reaction
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::UpdateMessage(
                            CreateInteractionResponseMessage::new()
                                .content(super::load_patchnote::get_patchnote()),
                        ),
                    )
                    .await
                {
                    error!("patchnote response failed: {:?}", why);
                }
            }
            _ => {
                if let Err(why) = command.delete_response(&ctx.http).await {
                    error!("delete_response failed: {:?}", why);
                }
            }
        }
    }
}

async fn failed_notice_msg(ctx: &Context, command: CommandInteraction) {
    if let Err(why) = command
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new().content("⚠️ 서버 정보를 불러오는 데 실패했습니다."),
        )
        .await
    {
        error!("Failed to reponse slash command: {:#?}", why);
    }
}

fn setup_embed() -> CreateEmbed {
    CreateEmbed::new()
        .title("이 서버에는 명령어가 아직 등록되어있지 않습니다.")
        .description("밑의 등록 버튼을 눌러 서버에 있는 명령어를 불러와 등록할 수 있어요")
        .color((255, 255, 255))
}

fn update_components(pressed: bool) -> [CreateButton; 3] {
    [
        CreateButton::new("update_cmds")
            .label("업데이트")
            .style(ButtonStyle::Primary)
            .disabled(pressed),
        CreateButton::new("show_patchnotes")
            .label("패치노트")
            .style(ButtonStyle::Secondary)
            .disabled(pressed),
        CreateButton::new("cancel_update")
            .label("안할래")
            .style(ButtonStyle::Danger)
            .disabled(pressed),
    ]
}
