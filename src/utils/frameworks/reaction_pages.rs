use serenity::{
    builder::{
        CreateActionRow, CreateButton, CreateEmbed, CreateInteractionResponse,
        CreateInteractionResponseMessage, EditInteractionResponse,
    },
    client::Context,
    futures::StreamExt,
    model::{
        application::{ButtonStyle, CommandInteraction},
        channel::ReactionType,
    },
};

use std::time::Duration;

use log::error;

#[derive(Debug)]
pub struct SkippableEmbed {
    total: usize,
    pub(crate) embedlist: Vec<CreateEmbed>,
    current_idx: usize,
    button_disable_option: (bool, bool, bool, bool),
}

impl SkippableEmbed {
    fn next(&mut self) {
        self.current_idx = if self.current_idx + 1 < self.total {
            self.current_idx + 1
        } else {
            self.current_idx
        }
    }

    fn prev(&mut self) {
        self.current_idx = if self.current_idx > 0 {
            self.current_idx - 1
        } else {
            self.current_idx
        }
    }

    fn skip_end(&mut self) {
        if self.total > 0 {
            self.current_idx = self.total - 1;
        }
    }

    fn skip_start(&mut self) {
        self.current_idx = 0;
    }

    fn check_disable_button(&mut self) {
        self.button_disable_option = if self.total == 0 {
            (true, true, true, true)
        } else if self.current_idx + 1 == self.total {
            (false, false, true, true)
        } else if self.current_idx == 0 {
            (true, true, false, false)
        } else {
            (false, false, false, false)
        }
    }
}

fn set_reaction_page_action_row(reactive_interaction: &mut SkippableEmbed) -> CreateActionRow {
    reactive_interaction.check_disable_button();

    CreateActionRow::Buttons(vec![
        CreateButton::new("to_start")
            .style(ButtonStyle::Secondary)
            .emoji("⏮️".parse::<ReactionType>().unwrap())
            .disabled(reactive_interaction.button_disable_option.0),
        CreateButton::new("previous")
            .style(ButtonStyle::Secondary)
            .emoji("⬅️".parse::<ReactionType>().unwrap())
            .disabled(reactive_interaction.button_disable_option.1),
        CreateButton::new("next")
            .style(ButtonStyle::Secondary)
            .emoji("➡️".parse::<ReactionType>().unwrap())
            .disabled(reactive_interaction.button_disable_option.2),
        CreateButton::new("to_end")
            .style(ButtonStyle::Secondary)
            .emoji("⏭️".parse::<ReactionType>().unwrap())
            .disabled(reactive_interaction.button_disable_option.3),
        CreateButton::new("remove")
            .style(ButtonStyle::Danger)
            .emoji("✖️".parse::<ReactionType>().unwrap()),
    ])
}

pub async fn reaction_pages(
    interaction: CommandInteraction,
    ctx: &Context,
    embeds: Vec<CreateEmbed>,
) -> Result<serenity::model::channel::Message, serenity::Error> {
    if embeds.is_empty() {
        return interaction
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new().content("표시할 내용이 없습니다."),
            )
            .await;
    }

    let mut reactive_interaction = SkippableEmbed {
        total: embeds.len(),
        embedlist: embeds.clone(),
        current_idx: 0,
        button_disable_option: (true, true, true, true),
    };

    if let Err(why) = interaction
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new()
                .components(vec![set_reaction_page_action_row(
                    &mut reactive_interaction,
                )])
                .embed(embeds[0].clone()),
        )
        .await
    {
        error!("an error occured while adding buttons.");
        return Err(why);
    };

    let msg = match interaction.get_response(&ctx.http).await {
        Ok(m) => m,
        Err(why) => {
            error!("Couldn't get message info from interaction.");
            return Err(why);
        }
    };

    let mut interaction_stream = msg
        .clone()
        .await_component_interactions(ctx)
        .timeout(Duration::from_secs(60 * 3))
        .filter({
            let cmd_user = interaction.user.id;
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

    while let Some(button_reaction) = interaction_stream.next().await {
        match button_reaction.data.custom_id.as_str() {
            "to_start" => reactive_interaction.skip_start(),
            "next" => reactive_interaction.next(),
            "previous" => reactive_interaction.prev(),
            "to_end" => reactive_interaction.skip_end(),
            "remove" => {
                if let Err(why) = msg.delete(&ctx.http).await {
                    error!("Couldn't delete message in reaction button invoking 'remove'.");
                    return Err(why);
                }
                return Ok(msg);
            }
            _ => continue,
        }

        if let Err(why) = button_reaction
            .create_response(
                &ctx.http,
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new()
                        .embed(
                            reactive_interaction.embedlist[reactive_interaction.current_idx]
                                .clone(),
                        )
                        .components(vec![set_reaction_page_action_row(
                            &mut reactive_interaction,
                        )]),
                ),
            )
            .await
        {
            error!("Couldn't set embed.");
            return Err(why);
        }
    }

    if let Err(why) = msg.delete(&ctx.http).await {
        error!("an error occured while deleting message.");
        return Err(why);
    };

    Ok(msg)
}
