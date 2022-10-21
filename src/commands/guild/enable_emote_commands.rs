use async_trait::async_trait;
use futures::{stream, StreamExt, TryStreamExt};
use log::*;
use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::{
        command::{CommandOptionType, CommandType},
        interaction::application_command::ApplicationCommandInteraction,
        GuildId,
    },
    prelude::Context,
};

use crate::{
    commands::{guild::emote_commands::is_commands_enabled, AppCmd},
    Handler, HandlerError,
};

use super::{emote_commands::GuildEmoteCommandIds, GuildCommands};

async fn enable_emote_commands(
    guild_id: GuildId,
    handler: &Handler,
    context: &Context,
) -> Result<(), HandlerError> {
    let command_ids: Vec<_> = stream::iter(handler.log_message_repo.emote_list())
        .then(|emote| async move {
            guild_id
                .create_application_command(context, |cmd| {
                    cmd.name(emote).create_option(|opt| {
                        opt.kind(CommandOptionType::Mentionable)
                            .name("target")
                            .description("Optional target for the emote")
                    })
                })
                .await
                .map(|c| c.id)
        })
        .try_collect()
        .await?;
    if let Some(command_ids_map) = context.data.write().await.get_mut::<GuildEmoteCommandIds>() {
        match command_ids_map.get(&guild_id) {
            Some(ids) if !ids.is_empty() => {
                // maybe should disable all and then reenable?
                warn!("tried to enable emote commands for guild {:?} but there was already data ({:?})", guild_id, ids);
                return Ok(());
            }
            _ => {}
        }
        command_ids_map.insert(guild_id, command_ids);
    } else {
        return Err(HandlerError::TypeMapNotFound);
    }

    Ok(())
}

pub struct EnableEmoteCommands;

#[async_trait]
impl AppCmd for EnableEmoteCommands {
    fn to_application_command() -> CreateApplicationCommand
    where
        Self: Sized,
    {
        let mut cmd = CreateApplicationCommand::default();
        cmd.name(GuildCommands::EnableEmoteCommands)
            .kind(CommandType::ChatInput)
            .description("Enable emote commands in this server (adds commands for every emote!)");
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
        trace!("enabling emote commands");
        let guild_id = if let Some(id) = cmd.guild_id {
            id
        } else {
            return Err(HandlerError::NotGuild);
        };

        trace!("finding guild settings");

        if is_commands_enabled(&context.data, guild_id).await? {
            cmd.create_interaction_response(context, |res| {
                res.interaction_response_data(|data| {
                    data.ephemeral(true)
                        .content("Guild commands are already enabled")
                })
            })
            .await?;
        } else {
            enable_emote_commands(guild_id, handler, context).await?;
            cmd.create_interaction_response(context, |res| {
                res.interaction_response_data(|data| {
                    data.ephemeral(true).content("Guild commands enabled!")
                })
            })
            .await?;
        }

        Ok(())
    }
}
