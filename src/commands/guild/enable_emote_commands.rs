use async_trait::async_trait;
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

use crate::{commands::AppCmd, db::models::DbGuild, Handler, HandlerError};

use super::GuildCommands;

async fn enable_emote_commands(
    guild_id: &GuildId,
    handler: &Handler,
    context: &Context,
) -> Result<(), HandlerError> {
    for emote in handler.log_message_repo.emote_list() {
        guild_id
            .create_application_command(context, |cmd| {
                cmd.name(emote).create_option(|opt| {
                    opt.kind(CommandOptionType::Mentionable)
                        .name("target")
                        .description("Optional target for the emote")
                })
            })
            .await?;
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
        let guild_id = if let Some(id) = cmd.guild_id {
            id
        } else {
            return Err(HandlerError::NotGuild);
        };

        trace!("finding guild settings");
        let discord_id = cmd.user.id.to_string();
        let guild = handler
            .db
            .find_guild(discord_id.clone())
            .await?
            .unwrap_or(DbGuild {
                discord_id,
                ..Default::default()
            });

        if guild.commands_enabled {
            cmd.create_interaction_response(context, |res| {
                res.interaction_response_data(|data| {
                    data.ephemeral(true)
                        .content("Guild commands are already enabled")
                })
            })
            .await?;
        } else {
            enable_emote_commands(&guild_id, handler, context).await?;
            handler
                .db
                .upsert_guild(guild.discord_id, guild.language, guild.gender, true)
                .await?;
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
