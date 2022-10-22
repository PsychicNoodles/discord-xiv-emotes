use async_trait::async_trait;
use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::interaction::application_command::ApplicationCommandInteraction,
    prelude::Context,
};
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, Display, EnumIter, EnumString};

use crate::{Handler, HandlerError};

use self::{
    emote::EmoteCmd, emote_select::EmoteSelectCmd, list_emotes::ListEmotesCmd,
    user_settings::UserSettingsCmd,
};

use super::{AppCmd, CommandsEnum};

pub mod emote;
pub mod emote_select;
pub mod list_emotes;
pub mod user_settings;

#[derive(Debug, Clone, Copy, AsRefStr, Display, EnumString, EnumIter)]
pub enum GlobalCommands {
    #[strum(serialize = "emote-select")]
    EmoteSelect,
    #[strum(serialize = "settings")]
    UserSettings,
    #[strum(serialize = "emote")]
    Emote,
    #[strum(serialize = "list-emotes")]
    ListEmotes,
}

impl GlobalCommands {
    pub fn to_application_command(self) -> CreateApplicationCommand {
        match self {
            GlobalCommands::EmoteSelect => EmoteSelectCmd::to_application_command(),
            GlobalCommands::UserSettings => UserSettingsCmd::to_application_command(),
            GlobalCommands::Emote => EmoteCmd::to_application_command(),
            GlobalCommands::ListEmotes => ListEmotesCmd::to_application_command(),
        }
    }

    pub fn application_commands() -> impl Iterator<Item = CreateApplicationCommand> {
        Self::iter().map(Self::to_application_command)
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
        }
        .await
    }
}
