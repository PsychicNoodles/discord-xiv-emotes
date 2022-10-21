use async_trait::async_trait;
use futures::{stream, StreamExt, TryStreamExt};
use log::*;
use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::{
        command::CommandType,
        interaction::{
            application_command::ApplicationCommandInteraction, InteractionResponseType,
        },
        GuildId,
    },
    prelude::Context,
};

use crate::{
    commands::{guild::emote_commands::is_commands_enabled, AppCmd},
    Handler, HandlerError,
};

use super::{emote_commands::GuildEmoteCommandIds, GuildCommands};

async fn disable_emote_commands(guild_id: GuildId, context: &Context) -> Result<(), HandlerError> {
    let command_ids: Vec<_> = if let Some(command_ids_map) =
        context.data.write().await.get_mut::<GuildEmoteCommandIds>()
    {
        match command_ids_map.get_mut(&guild_id) {
            Some(ids) if ids.is_empty() => {
                warn!(
                    "tried to disable emote commands for guild {:?} but command id list was empty",
                    guild_id
                );
                return Ok(());
            }
            None => {
                warn!(
                    "tried to disable emote commands for guild {:?} but there was no data",
                    guild_id
                );
                return Ok(());
            }
            // collect so that the data is owned before dropping lock
            Some(ids) => ids.drain(..).collect(),
        }
    } else {
        return Err(HandlerError::TypeMapNotFound);
    };

    stream::iter(command_ids)
        .then(|id| async move { guild_id.delete_application_command(context, id).await })
        .try_collect()
        .await?;

    Ok(())
}

pub struct DisableEmoteCommands;

#[async_trait]
impl AppCmd for DisableEmoteCommands {
    fn to_application_command() -> CreateApplicationCommand
    where
        Self: Sized,
    {
        let mut cmd = CreateApplicationCommand::default();
        cmd.name(GuildCommands::DisableEmoteCommands)
            .kind(CommandType::ChatInput)
            .description("Disable emote commands in this server");
        cmd
    }

    async fn handle(
        cmd: &ApplicationCommandInteraction,
        _handler: &Handler,
        context: &Context,
    ) -> Result<(), HandlerError>
    where
        Self: Sized,
    {
        trace!("disabling emote commands");
        let guild_id = if let Some(id) = cmd.guild_id {
            id
        } else {
            return Err(HandlerError::NotGuild);
        };

        trace!("finding guild settings");

        if is_commands_enabled(&context.data, guild_id).await? {
            trace!("disabling commands");
            cmd.create_interaction_response(context, |res| {
                res.kind(InteractionResponseType::DeferredChannelMessageWithSource)
            })
            .await?;
            disable_emote_commands(guild_id, context).await?;
            trace!("finished disabling commands");
            cmd.create_interaction_response(context, |res| {
                res.interaction_response_data(|data| {
                    data.ephemeral(true).content("Guild commands disabled!")
                })
            })
            .await?;
        } else {
            trace!("commands are already disabled");
            cmd.create_interaction_response(context, |res| {
                res.interaction_response_data(|data| {
                    data.ephemeral(true)
                        .content("Guild commands are already disabled")
                })
            })
            .await?;
        }

        Ok(())
    }
}
