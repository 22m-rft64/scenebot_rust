use serenity::{
    async_trait,
    builder::{CreateCommand, CreateEmbed, EditInteractionResponse},
    client::Context,
    model::{
        application::{CommandDataOption, CommandInteraction},
        prelude::Message,
    },
    Error,
};

use crate::command_handler::explicit_command_list::CommandInterface;

const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");
const SHOW_ENTRIES: usize = 3;

struct Changelog;

pub fn get_command() -> Box<dyn CommandInterface + Sync + Send> {
    Box::new(Changelog)
}

// "## 날짜" 단위로 끊어서 최신 몇 개만
fn changelog_embed() -> CreateEmbed {
    let fields: Vec<(String, String, bool)> = CHANGELOG
        .split("## ")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .take(SHOW_ENTRIES)
        .map(|entry| {
            let (title, body) = entry.split_once('\n').unwrap_or((entry, ""));
            (title.trim().to_string(), body.trim().to_string(), false)
        })
        .collect();

    CreateEmbed::default().title("업데이트 내역").fields(fields)
}

#[async_trait]
impl CommandInterface for Changelog {
    async fn run(
        &self,
        ctx: &Context,
        _options: &[CommandDataOption],
        command: &CommandInteraction,
    ) -> Result<Message, Error> {
        command
            .edit_response(
                &ctx.http,
                EditInteractionResponse::default().embed(changelog_embed()),
            )
            .await
    }

    fn name(&self) -> String {
        String::from("changelog")
    }

    fn register(&self) -> CreateCommand {
        CreateCommand::new(self.name()).description("봇 업데이트 내역을 보여줘요")
    }

    fn ephemeral(&self) -> bool {
        true
    }
}
