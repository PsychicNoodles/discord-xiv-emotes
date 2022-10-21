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
    disable_emote_commands::DisableEmoteCommands, enable_emote_commands::EnableEmoteCommands,
};

use super::{AppCmd, CommandsEnum};

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

    pub fn application_commands() -> impl Iterator<Item = CreateApplicationCommand> {
        Self::iter().map(Self::to_application_command)
    }
}

#[async_trait]
impl CommandsEnum for GuildCommands {
    async fn handle(
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

pub mod emote_commands {
    use std::{collections::HashMap, sync::Arc};

    use serenity::{
        model::prelude::{CommandId, GuildId},
        prelude::{RwLock, TypeMap, TypeMapKey},
    };

    use crate::HandlerError;

    pub struct GuildEmoteCommandIds;

    impl TypeMapKey for GuildEmoteCommandIds {
        type Value = HashMap<GuildId, Vec<CommandId>>;
    }

    pub async fn is_commands_enabled(
        data: &Arc<RwLock<TypeMap>>,
        guild_id: GuildId,
    ) -> Result<bool, HandlerError> {
        data.read()
            .await
            .get::<GuildEmoteCommandIds>()
            .ok_or(HandlerError::TypeMapNotFound)
            .map(|map| map.get(&guild_id).map(|v| !v.is_empty()).unwrap_or(false))
    }
}
