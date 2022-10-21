use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::interaction::application_command::ApplicationCommandInteraction,
    prelude::Context,
};
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, Display, EnumIter, EnumString};

use crate::{Handler, HandlerError};

use self::enable_guild_commands::EnableGuildCommands;

use super::AppCmd;

pub mod enable_guild_commands;

#[derive(Debug, Clone, Copy, AsRefStr, Display, EnumString, EnumIter)]
pub enum GuildCommands {
    #[strum(serialize = "enable-commands")]
    EnableCommands,
}

impl GuildCommands {
    pub fn to_application_command(self) -> CreateApplicationCommand {
        match self {
            GuildCommands::EnableCommands => EnableGuildCommands::to_application_command(),
        }
    }

    pub fn application_commands() -> Vec<CreateApplicationCommand> {
        Self::iter().map(Self::to_application_command).collect()
    }

    pub async fn handle(
        self,
        cmd: &ApplicationCommandInteraction,
        handler: &Handler,
        context: &Context,
    ) -> Result<(), HandlerError> {
        match self {
            GuildCommands::EnableCommands => EnableGuildCommands::handle(cmd, handler, context),
        }
        .await
    }
}
