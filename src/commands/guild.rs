use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::interaction::application_command::ApplicationCommandInteraction,
    prelude::Context,
};
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, Display, EnumIter, EnumString};

use crate::{Handler, HandlerError};

use self::{
    disable_emote_commands::DisableEmoteCommands, enable_emote_commands::EnableEmoteCommands,
};

use super::AppCmd;

pub mod disable_emote_commands;
pub mod enable_emote_commands;

#[derive(Debug, Clone, Copy, AsRefStr, Display, EnumString, EnumIter)]
pub enum GuildCommands {
    #[strum(serialize = "enable-emote-commands")]
    EnableEmoteCommands,
    #[strum(serialize = "disable-emote-commands")]
    DisableEmoteCommands,
}

impl GuildCommands {
    pub fn to_application_command(self) -> CreateApplicationCommand {
        match self {
            GuildCommands::EnableEmoteCommands => EnableEmoteCommands::to_application_command(),
            GuildCommands::DisableEmoteCommands => DisableEmoteCommands::to_application_command(),
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
            GuildCommands::EnableEmoteCommands => {
                EnableEmoteCommands::handle(cmd, handler, context)
            }
            GuildCommands::DisableEmoteCommands => {
                DisableEmoteCommands::handle(cmd, handler, context)
            }
        }
        .await
    }
}
