use async_trait::async_trait;
use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::{
        command::CommandType, interaction::application_command::ApplicationCommandInteraction,
    },
    prelude::Context,
};

use crate::{commands::AppCmd, Handler, HandlerError};

use super::GuildCommands;

pub struct EnableGuildCommands;

#[async_trait]
impl AppCmd for EnableGuildCommands {
    fn to_application_command() -> CreateApplicationCommand
    where
        Self: Sized,
    {
        let mut cmd = CreateApplicationCommand::default();
        cmd.name(GuildCommands::EnableCommands)
            .kind(CommandType::ChatInput)
            .description("Enable guild commands in this server (adds commands for every emote!)");
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
        todo!()
    }
}
