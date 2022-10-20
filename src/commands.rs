use async_trait::async_trait;
use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::interaction::application_command::ApplicationCommandInteraction,
    prelude::Context,
};
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, Display, EnumIter, EnumString};

use crate::{Handler, HandlerError};

use self::{emote_select::EmoteSelectCmd, user_settings::UserSettingsCmd};

pub mod emote_select;
pub mod user_settings;

#[derive(Debug, Clone, Copy, AsRefStr, Display, EnumString, EnumIter)]
pub enum Commands {
    #[strum(serialize = "emote")]
    EmoteSelect,
    #[strum(serialize = "settings")]
    UserSettings,
}

#[async_trait]
trait AppCmd {
    fn to_application_command() -> CreateApplicationCommand
    where
        Self: Sized;
    async fn handle(
        cmd: &ApplicationCommandInteraction,
        handler: &Handler,
        context: &Context,
    ) -> Result<(), HandlerError>
    where
        Self: Sized;
}

impl Commands {
    pub fn to_application_command(self) -> CreateApplicationCommand {
        match self {
            Commands::EmoteSelect => EmoteSelectCmd::to_application_command(),
            Commands::UserSettings => UserSettingsCmd::to_application_command(),
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
            Commands::EmoteSelect => EmoteSelectCmd::handle(cmd, handler, context),
            Commands::UserSettings => UserSettingsCmd::handle(cmd, handler, context),
        }
        .await
    }
}
