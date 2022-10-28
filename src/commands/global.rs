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

use self::{
    emote::EmoteCmd, emote_select::EmoteSelectCmd, list_emotes::ListEmotesCmd,
    stats::GlobalStatsCmd, user_settings::UserSettingsCmd,
};

use super::{AppCmd, CommandsEnum};

pub mod emote;
pub mod emote_select;
pub mod list_emotes;
pub mod stats;
pub mod user_settings;

#[derive(Debug, Clone, Copy, AsRefStr, Display, EnumIter, PartialEq, Eq, Hash)]
pub enum GlobalCommands {
    EmoteSelect,
    UserSettings,
    Emote,
    ListEmotes,
    Stats,
}

impl GlobalCommands {
    pub fn to_application_command(self) -> CreateApplicationCommand {
        match self {
            GlobalCommands::EmoteSelect => EmoteSelectCmd::to_application_command(),
            GlobalCommands::UserSettings => UserSettingsCmd::to_application_command(),
            GlobalCommands::Emote => EmoteCmd::to_application_command(),
            GlobalCommands::ListEmotes => ListEmotesCmd::to_application_command(),
            GlobalCommands::Stats => GlobalStatsCmd::to_application_command(),
        }
    }

    pub fn application_commands() -> impl Iterator<Item = CreateApplicationCommand> {
        Self::iter().map(Self::to_application_command)
    }

    pub fn name(self) -> LocalizedString {
        match self {
            GlobalCommands::EmoteSelect => EmoteSelectCmd::name(),
            GlobalCommands::UserSettings => UserSettingsCmd::name(),
            GlobalCommands::Emote => EmoteCmd::name(),
            GlobalCommands::ListEmotes => ListEmotesCmd::name(),
            GlobalCommands::Stats => GlobalStatsCmd::name(),
        }
    }
}

#[async_trait]
impl CommandsEnum for GlobalCommands {
    async fn handle(
        self,
        cmd: &ApplicationCommandInteraction,
        handler: &Handler,
        context: &Context,
        message_db_data: &MessageDbData,
    ) -> Result<(), HandlerError> {
        match self {
            GlobalCommands::EmoteSelect => {
                EmoteSelectCmd::handle(cmd, handler, context, message_db_data)
            }
            GlobalCommands::UserSettings => {
                UserSettingsCmd::handle(cmd, handler, context, message_db_data)
            }
            GlobalCommands::Emote => EmoteCmd::handle(cmd, handler, context, message_db_data),
            GlobalCommands::ListEmotes => {
                ListEmotesCmd::handle(cmd, handler, context, message_db_data)
            }
            GlobalCommands::Stats => GlobalStatsCmd::handle(cmd, handler, context, message_db_data),
        }
        .await
    }
}

impl TypeMapKey for GlobalCommands {
    type Value = HashMap<CommandId, Self>;
}

#[derive(Debug, Clone, Error)]
#[error("Not a valid command: {0}")]
pub struct InvalidGlobalCommand(String);

impl FromStr for GlobalCommands {
    type Err = InvalidGlobalCommand;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        GlobalCommands::iter()
            .find(|cmd| cmd.name().any_eq(s))
            .ok_or_else(|| InvalidGlobalCommand(s.to_string()))
    }
}
