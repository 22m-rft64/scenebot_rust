use lazy_static::lazy_static;
use log::error;
use serenity::{
    async_trait,
    builder::CreateCommand,
    client::Context,
    model::{
        application::{CommandDataOption, CommandInteraction},
        id::GuildId,
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
}

pub struct CommandList {
    pub commands: HashMap<&'static str, Box<dyn CommandInterface + Send + Sync>>,
}

impl CommandList {
    pub async fn register_commands(&'static self, gid: GuildId, ctx: &Context) {
        for cmd in self.commands.values() {
            if let Err(why) = gid.create_command(&ctx.http, cmd.register()).await {
                error!("Couldn't create application command: {:#?}", why);
            }
        }
    }
}

//명령어 만든거를 여기에 등록시킴.
//개발 끝난거면 여따 쓰면 되니까 개발중/개발완료를 구분할 수 있음
lazy_static! {
    pub static ref COMMAND_LIST: CommandList = CommandList {
        commands: HashMap::from([
            ("send", commands::send::get_command()),
            ("config", commands::config::get_command()),
            ("help", commands::help::get_command()),
            //("webp", commands::webp_transfer::get_command())
        ])
    };
}
