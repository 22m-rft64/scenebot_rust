use log::error;
use serenity::{client::Context, model::id::GuildId};

use super::super::explicit_command_list::COMMAND_LIST;

pub enum UpdateStatus {
    FirstSetting,
    UpdateAvailable(Vec<String>),
    LatestVersion,
    FailedtoLoad,
}

pub async fn check_updates(ctx: &Context, gid: GuildId) -> UpdateStatus {
    match gid.get_commands(&ctx.http).await {
        Ok(cmds) => {
            let mut guild_cmds_list = cmds
                .iter()
                .map(|v| v.name.clone())
                .collect::<Vec<String>>();
            let mut server_cmds_list = Vec::new();
            for scmds in COMMAND_LIST.commands.values() {
                server_cmds_list.push(scmds.name());
            }
            server_cmds_list.push("update".to_string());

            guild_cmds_list.sort();
            server_cmds_list.sort();

            if guild_cmds_list == vec!["update".to_string()] {
                UpdateStatus::FirstSetting
            } else if guild_cmds_list == server_cmds_list {
                UpdateStatus::LatestVersion
            } else {
                let mut unassigned_commands = Vec::new();
                for cmdname in COMMAND_LIST.commands.keys() {
                    if !guild_cmds_list.contains(&cmdname.to_string()) {
                        unassigned_commands.push(cmdname.to_string());
                    }
                }
                UpdateStatus::UpdateAvailable(unassigned_commands)
            }
        }
        Err(why) => {
            error!(
                "Failed to get application data from {}. \nwhy: {:#?}",
                gid, why
            );
            UpdateStatus::FailedtoLoad
        }
    }
}
