//Interaction 중 ApplicationCommand가 emit되면 실행되는 파일
//music 관련 명령어인 경우에는 music_modules/command_handler/handler.rs로 넘기지만
//여기가 메인 핸들러라고 생각하면 됨.
use log::error;
use serenity::{
    builder::EditInteractionResponse, client::Context, model::application::CommandInteraction,
};

use super::explicit_command_list::COMMAND_LIST;

pub async fn seperate_command(command: CommandInteraction, ctx: &Context) {
    if let Err(why) = command.defer(&ctx.http).await {
        error!("defer failed for {}: {:?}", command.data.name, why);
        return;
    }

    let cmd_result = match COMMAND_LIST.commands.get(command.data.name.as_str()) {
        Some(exist_command) => {
            exist_command
                .run(ctx, &command.data.options, &command)
                .await
        }
        None => {
            command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().content("아직 구현되지 않은 명령어입니다."),
                )
                .await
        }
    };

    if let Err(why) = cmd_result {
        error!("an error occured while responding command : {:#?}", why);
    }
}
