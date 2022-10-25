use async_trait::async_trait;
use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::interaction::application_command::ApplicationCommandInteraction,
    prelude::Context,
};
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, Display, EnumIter, EnumString};

use crate::{Handler, HandlerError, MessageDbData};

use super::CommandsEnum;

#[derive(Debug, Clone, Copy, AsRefStr, Display, EnumString, EnumIter)]
pub enum GuildCommands {}

impl GuildCommands {
    pub fn to_application_command(self) -> CreateApplicationCommand {
        match self {}
    }

    pub fn application_commands() -> impl Iterator<Item = CreateApplicationCommand> {
        Self::iter().map(Self::to_application_command)
    }
}

#[async_trait]
impl CommandsEnum for GuildCommands {
    async fn handle(
        self,
        _cmd: &ApplicationCommandInteraction,
        _handler: &Handler,
        _context: &Context,
        _message_db_data: &MessageDbData,
    ) -> Result<(), HandlerError> {
        // match self {}.await
        Ok(())
    }
}
