pub mod server_settings;
pub mod stats;

use std::{collections::HashMap, str::FromStr};

use async_trait::async_trait;
use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::{interaction::application_command::ApplicationCommandInteraction, CommandId},
    prelude::{Context, TypeMapKey},
};
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, Display, EnumIter};
use thiserror::Error;

use crate::{util::LocalizedString, Handler, HandlerError, MessageDbData};

use self::{server_settings::ServerSettingsCmd, stats::GuildStatsCmd};

use super::{AppCmd, CommandsEnum};

#[derive(Debug, Clone, Copy, AsRefStr, Display, EnumIter, PartialEq, Eq, Hash)]
pub enum GuildCommands {
    ServerSettings,
    Stats,
}

impl GuildCommands {
    pub fn to_application_command(self) -> CreateApplicationCommand {
        match self {
            GuildCommands::ServerSettings => ServerSettingsCmd::to_application_command(),
            GuildCommands::Stats => GuildStatsCmd::to_application_command(),
        }
    }

    pub fn application_commands() -> impl Iterator<Item = CreateApplicationCommand> {
        Self::iter().map(Self::to_application_command)
    }

    pub fn name(self) -> LocalizedString {
        match self {
            GuildCommands::ServerSettings => ServerSettingsCmd::name(),
            GuildCommands::Stats => GuildStatsCmd::name(),
        }
    }
}

#[async_trait]
impl CommandsEnum for GuildCommands {
    async fn handle(
        self,
        cmd: &ApplicationCommandInteraction,
        handler: &Handler,
        context: &Context,
        message_db_data: &MessageDbData,
    ) -> Result<(), HandlerError> {
        match self {
            GuildCommands::ServerSettings => {
                ServerSettingsCmd::handle(cmd, handler, context, message_db_data)
            }
            GuildCommands::Stats => GuildStatsCmd::handle(cmd, handler, context, message_db_data),
        }
        .await
    }
}

impl TypeMapKey for GuildCommands {
    type Value = HashMap<CommandId, Self>;
}

#[derive(Debug, Clone, Error)]
#[error("Not a valid command: {0}")]
pub struct InvalidGuildCommand(String);

impl FromStr for GuildCommands {
    type Err = InvalidGuildCommand;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        GuildCommands::iter()
            .find(|cmd| cmd.name().any_eq(s))
            .ok_or_else(|| InvalidGuildCommand(s.to_string()))
    }
}
