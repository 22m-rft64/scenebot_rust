use serenity::{
    async_trait,
    builder::{CreateCommand, CreateCommandOption, EditInteractionResponse},
    client::Context,
    model::{
        application::{CommandDataOption, CommandInteraction, CommandOptionType},
        prelude::Message,
    },
    Error,
};

use crate::command_handler::explicit_command_list::CommandInterface;
use crate::utils::scene_core::{emoji_format_filter, get_resized_image, ImageSize};

struct SendSizedEmoji;

pub fn get_command() -> Box<dyn CommandInterface + Sync + Send> {
    Box::new(SendSizedEmoji)
}

#[async_trait]
impl CommandInterface for SendSizedEmoji {
    async fn run(
        &self,
        ctx: &Context,
        options: &[CommandDataOption],
        command: &CommandInteraction,
    ) -> Result<Message, Error> {
        let emoji = options.first().and_then(|o| o.value.as_str());
        let size_num = options.get(1).and_then(|o| o.value.as_i64());

        let (Some(emoji), Some(size_num)) = (emoji, size_num) else {
            return command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::default()
                        .content("명령어를 조건에 맞게 입력해주세요"),
                )
                .await;
        };

        let filtered = match emoji_format_filter(emoji) {
            Ok(f) => f,
            Err(()) => {
                return command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::default()
                            .content("제대로 된 이모지를 입력해주세요"),
                    )
                    .await;
            }
        };

        let (is_png, img_url) = filtered;
        let target_size = if !is_png {
            ImageSize::Auto
        } else {
            match size_num {
                0 => ImageSize::HyperTechniqueOfLisaSuFinger,
                1 => ImageSize::Small,
                2 => ImageSize::Medium,
                3 => ImageSize::Large,
                4 => ImageSize::HyperSuperUltraSexFeaturedFuckingLarge,
                _ => ImageSize::Auto,
            }
        };

        match get_resized_image(ctx, img_url.as_ref(), &target_size).await {
            Some(resized_emoji) => {
                command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::default().new_attachment(resized_emoji),
                    )
                    .await
            }
            None => {
                command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::default()
                            .content("이모지를 변환하는 데 실패했습니다."),
                    )
                    .await
            }
        }
    }

    fn name(&self) -> String {
        String::from("send")
    }

    fn register(&self) -> CreateCommand {
        let options = Vec::from([
            CreateCommandOption::new(
                CommandOptionType::String,
                "emoji",
                "보낼 이모지를 선택해주세요.",
            )
            .required(true),
            CreateCommandOption::new(
                CommandOptionType::Integer,
                "size",
                "이모지의 크기를 정해주세요. 0~4까지의 숫자로 표현되고, 0은 가장 작은 크기입니다.",
            )
            .min_int_value(0)
            .max_int_value(4)
            .required(true),
        ]);
        CreateCommand::new(self.name())
            .description("이모지의 크기를 변경해 전송합니다")
            .set_options(options)
    }
}
