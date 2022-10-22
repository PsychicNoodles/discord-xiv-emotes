use std::borrow::Cow;

use async_trait::async_trait;
use log::*;
use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::{
        command::{CommandOptionType, CommandType},
        interaction::application_command::{ApplicationCommandInteraction, CommandData},
    },
    prelude::{Context, Mentionable},
};

use crate::{commands::AppCmd, Handler, HandlerError, PREFIX};

use super::GlobalCommands;

fn resolve_mention(data: &CommandData, context: &Context) -> Option<String> {
    if let Some(member) = data.resolved.members.values().next() {
        debug!("resolved to member");
        match (member.user.as_ref(), data.guild_id) {
            (Some(user), Some(guild_id)) => context
                .cache
                .member_field(guild_id, user.id, |mem| mem.mention().to_string()),
            (None, _) => {
                warn!("member did not have user data");
                None
            }
            (_, None) => {
                warn!("not in a guild");
                None
            }
        }
    } else if let Some(user) = data.resolved.users.values().next() {
        debug!("resolved to user");
        Some(user.mention().to_string())
    } else if let Some(role) = data.resolved.roles.values().next() {
        debug!("resolved to role");
        Some(role.mention().to_string())
    } else if let Some(channel) = data.resolved.channels.values().next() {
        debug!("resolved to channel");
        context
            .cache
            .channel(channel.id)
            .map(|c| c.mention().to_string())
    } else {
        None
    }
}

pub struct EmoteCmd;

#[async_trait]
impl AppCmd for EmoteCmd {
    fn to_application_command() -> CreateApplicationCommand
    where
        Self: Sized,
    {
        let mut cmd = CreateApplicationCommand::default();
        cmd.name(GlobalCommands::Emote)
            .kind(CommandType::ChatInput)
            .description("Send an emote with an optional target user")
            .create_option(|opt| {
                opt.kind(CommandOptionType::String)
                    .name("emote")
                    .description("Which emote to send")
                    .required(true)
            })
            .create_option(|opt| {
                opt.kind(CommandOptionType::Mentionable)
                    .name("target")
                    .description("Who to target with the emote")
            })
            .dm_permission(true);
        cmd
    }

    async fn handle(
        cmd: &ApplicationCommandInteraction,
        handler: &Handler,
        context: &Context,
    ) -> Result<(), HandlerError>
    where
        Self: Sized,
    {
        let emote = &cmd
            .data
            .options
            .get(0)
            .and_then(|o| o.value.as_ref())
            .and_then(|v| v.as_str())
            .ok_or(HandlerError::UnexpectedData)?;
        let target = resolve_mention(&cmd.data, context);
        trace!("target is {:?}", target);

        let emote = match emote.get(0..0) {
            None => {
                error!("emote is empty");
                return Err(HandlerError::UnrecognizedEmote("(empty)".to_string()));
            }
            Some("/") => Cow::Borrowed(*emote),
            Some(PREFIX) => Cow::Borrowed(emote.trim_start_matches(PREFIX)),
            Some(_) => Cow::Owned(["/", emote].concat()),
        };
        trace!("checking if emote exists: {:?}", emote);
        if !handler.log_message_repo.contains_emote(&emote) {
            cmd.create_interaction_response(context, |res| {
                res.interaction_response_data(|data| {
                    data.ephemeral(true).content(
                        "That's not a valid emote! Check the list of known emotes with /emotes",
                    )
                })
            })
            .await?;
            return Ok(());
        }

        let user = handler.db.find_user(cmd.user.id).await?.unwrap_or_default();

        let body = handler.build_emote_message(&emote, &user, &cmd.user, target.as_deref())?;
        cmd.channel_id
            .send_message(context, |m| m.content(body))
            .await?;

        cmd.create_interaction_response(context, |res| {
            res.interaction_response_data(|d| {
                d.ephemeral(true).content(format!(
                    "Emote sent! ({}{})",
                    emote,
                    if let Some(t) = &target {
                        [" ".to_string(), t.to_string()].concat()
                    } else {
                        "".to_string()
                    }
                ))
            })
        })
        .await?;

        Ok(())
    }
}
