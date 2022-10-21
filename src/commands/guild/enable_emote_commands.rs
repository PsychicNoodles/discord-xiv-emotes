use async_trait::async_trait;
use log::*;
use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::{
        command::{CommandOptionType, CommandType},
        interaction::{
            application_command::ApplicationCommandInteraction, InteractionResponseType,
        },
        GuildId,
    },
    prelude::Context,
};

use crate::{
    commands::{check_is_app_command_cap_err, guild::emote_commands::is_commands_enabled, AppCmd},
    Handler, HandlerError,
};

use super::{emote_commands::GuildEmoteCommandIds, GuildCommands};

async fn enable_emote_commands(
    guild_id: GuildId,
    handler: &Handler,
    context: &Context,
) -> Result<(), HandlerError> {
    let emotes: Vec<_> = handler
        .log_message_repo
        .emote_list_by_id()
        .map(|emote| emote.strip_prefix("/").unwrap_or(emote))
        .collect();

    let emote_commands = emotes.iter().map(|emote| {
        let mut cmd = CreateApplicationCommand::default();
        cmd.name(emote)
            .kind(CommandType::ChatInput)
            .description(format!("Use the {} emote", emote))
            .create_option(|opt| {
                opt.kind(CommandOptionType::Mentionable)
                    .name("target")
                    .description("Optional target for the emote")
            });
        cmd
    });
    let const_commands = GuildCommands::application_commands();

    let command_ids = guild_id
        .set_application_commands(context, |set| {
            set.set_application_commands(emote_commands.chain(const_commands).collect())
        })
        .await
        .map_err(check_is_app_command_cap_err)?;
    // unfortunately the most unique info for an application command is the name, but it's also unique so good enough
    let command_ids = command_ids
        .into_iter()
        .filter(|c| emotes.contains(&c.name.as_str()))
        .map(|c| c.id)
        .collect();

    if let Some(command_ids_map) = context.data.write().await.get_mut::<GuildEmoteCommandIds>() {
        trace!("got command ids map");
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
            trace!("commands are already enabled");
            cmd.create_interaction_response(context, |res| {
                res.interaction_response_data(|data| {
                    data.ephemeral(true)
                        .content("Guild commands are already enabled")
                })
            })
            .await?;
        } else {
            trace!("enabling commands");
            cmd.create_interaction_response(context, |res| {
                res.kind(InteractionResponseType::DeferredChannelMessageWithSource)
            })
            .await?;
            enable_emote_commands(guild_id, handler, context).await?;
            trace!("finished enabling commands");
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
