use lazy_static::lazy_static;
use log::error;
use serenity::{
    async_trait,
    builder::CreateCommand,
    client::Context,
    model::{
        application::{
            Command, CommandDataOption, CommandInteraction, InstallationContext,
            InteractionContext,
        },
        prelude::Message,
    },
    Error,
};

use std::collections::HashMap;

use super::commands;

#[async_trait]
pub trait CommandInterface {
    async fn run(
        &self,
        ctx: &Context,
        options: &[CommandDataOption],
        command: &CommandInteraction,
    ) -> Result<Message, Error>;

    fn name(&self) -> String;

    fn register(&self) -> CreateCommand;

    // true면 명령어 친 사람한테만 보임
    fn ephemeral(&self) -> bool {
        false
    }
}

pub struct CommandList {
    pub commands: HashMap<&'static str, Box<dyn CommandInterface + Send + Sync>>,
}

impl CommandList {
    // 글로벌로 한 번에 덮어씀. 서버에서만 보이게 DM/유저설치는 막음.
    pub async fn register_global(&'static self, ctx: &Context) {
        let commands: Vec<CreateCommand> = self
            .commands
            .values()
            .map(|cmd| {
                cmd.register()
                    .contexts(vec![InteractionContext::Guild])
                    .integration_types(vec![InstallationContext::Guild])
            })
            .collect();
        if let Err(why) = Command::set_global_commands(&ctx.http, commands).await {
            error!("Couldn't set global commands: {:#?}", why);
        }
    }
}

//명령어 만든거를 여기에 등록시킴.
//개발 끝난거면 여따 쓰면 되니까 개발중/개발완료를 구분할 수 있음
lazy_static! {
    pub static ref COMMAND_LIST: CommandList = CommandList {
        commands: HashMap::from([
            ("config", commands::config::get_command()),
            ("changelog", commands::changelog::get_command()),
        ])
    };
}
