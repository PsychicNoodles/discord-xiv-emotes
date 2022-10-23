use std::str::FromStr;

use async_trait::async_trait;
use log::*;
use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::interaction::application_command::ApplicationCommandInteraction,
    prelude::Context,
};
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, Display, EnumIter};
use thiserror::Error;

use crate::{util::LocalizedString, Handler, HandlerError};

use self::{
    emote::EmoteCmd, emote_select::EmoteSelectCmd, list_emotes::ListEmotesCmd,
    server_settings::ServerSettingsCmd, user_settings::UserSettingsCmd,
};

use super::{AppCmd, CommandsEnum};

pub mod emote;
pub mod emote_select;
pub mod list_emotes;
pub mod server_settings;
pub mod user_settings;

#[derive(Debug, Clone, Copy, AsRefStr, Display, EnumIter)]
pub enum GlobalCommands {
    EmoteSelect,
    UserSettings,
    Emote,
    ListEmotes,
    ServerSettings,
}

impl GlobalCommands {
    pub fn to_application_command(self) -> CreateApplicationCommand {
        match self {
            GlobalCommands::EmoteSelect => EmoteSelectCmd::to_application_command(),
            GlobalCommands::UserSettings => UserSettingsCmd::to_application_command(),
            GlobalCommands::Emote => EmoteCmd::to_application_command(),
            GlobalCommands::ListEmotes => ListEmotesCmd::to_application_command(),
            GlobalCommands::ServerSettings => ServerSettingsCmd::to_application_command(),
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
            GlobalCommands::ServerSettings => ServerSettingsCmd::name(),
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
    ) -> Result<(), HandlerError> {
        match self {
            GlobalCommands::EmoteSelect => EmoteSelectCmd::handle(cmd, handler, context),
            GlobalCommands::UserSettings => UserSettingsCmd::handle(cmd, handler, context),
            GlobalCommands::Emote => EmoteCmd::handle(cmd, handler, context),
            GlobalCommands::ListEmotes => ListEmotesCmd::handle(cmd, handler, context),
            GlobalCommands::ServerSettings => ServerSettingsCmd::handle(cmd, handler, context),
        }
        .await
    }
}

#[derive(Debug, Clone, Error)]
#[error("Not a valid command: {0}")]
pub struct InvalidGlobalCommand(String);

impl FromStr for GlobalCommands {
    type Err = InvalidGlobalCommand;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        trace!("checking for global command: {}", s);
        GlobalCommands::iter()
            .find(|cmd| cmd.name().any_eq(s))
            .ok_or_else(|| InvalidGlobalCommand(s.to_string()))
    }
}
