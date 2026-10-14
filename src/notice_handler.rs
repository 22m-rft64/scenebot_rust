// NOTICE_MODE=1 일 때 쓰는 핸들러.
// 이렘은 Message Content intent가 없어서 이모지를 못 읽음 → 명령어 치면 씬 초대 안내만 함.
use log::{error, info};
use serenity::{
    async_trait,
    builder::{
        CreateActionRow, CreateButton, CreateEmbed, CreateInteractionResponse,
        CreateInteractionResponseMessage,
    },
    client::{Context, EventHandler},
    gateway::ActivityData,
    model::{application::Interaction, gateway::Ready},
};

use crate::command_handler::explicit_command_list::COMMAND_LIST;

const SCENE_INVITE_URL: &str = "https://discord.com/api/oauth2/authorize?client_id=887415276117827665&permissions=8&scope=bot%20applications.commands";

pub struct NoticeHandler;

#[async_trait]
impl EventHandler for NoticeHandler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{}으로 로그인 완료! (안내 모드)", ready.user.tag());
        ctx.set_activity(Some(ActivityData::playing("씬봇으로 이사갔어요 | 명령어를 쳐보세요")));
        COMMAND_LIST.register_global(&ctx).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::Command(command) = interaction else {
            return;
        };

        let embed = CreateEmbed::default()
            .title("이 봇은 더 이상 지원되지 않아요")
            .description(
                "디스코드 정책 때문에 이 봇은 더 이상 이모지를 확대할 수 없어요.\n\
                 같은 기능을 하는 **씬**을 초대해서 계속 사용해주세요!",
            );
        let button = CreateButton::new_link(SCENE_INVITE_URL).label("씬 초대하기");

        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .embed(embed)
                .components(vec![CreateActionRow::Buttons(vec![button])])
                .ephemeral(true),
        );

        if let Err(why) = command.create_response(&ctx.http, response).await {
            error!("couldn't send notice: {:?}", why);
        }
    }
}
