use std::collections::HashMap;

use futures::{future::try_join_all, stream, StreamExt, TryStreamExt};
use serenity::{
    model::prelude::{command::Command, Ready},
    prelude::Context,
};
use tracing::*;

use crate::commands::{global::GlobalCommands, guild::GuildCommands, CommandsEnum};

use super::{Handler, HandlerError};

impl Handler {
    async fn save_command_ids<T>(
        &self,
        context: &Context,
        commands: impl Iterator<Item = Command>,
    ) -> Result<(), HandlerError>
    where
        T: CommandsEnum,
    {
        let mut cmd_map = HashMap::new();
        for cmd in commands {
            let cmd_enum =
                T::from_str(&cmd.name).map_err(|_| HandlerError::CommandRegisterUnknown)?;
            if let Some(prev) = cmd_map.insert(cmd.id, cmd_enum) {
                warn!(?prev, "overwrote previous command with same id");
            }
        }
        context.data.write().await.insert::<T>(cmd_map);
        Ok(())
    }

    pub async fn setup_global_commands(&self, context: &Context) -> Result<(), HandlerError> {
        let global_commands = match Command::set_global_application_commands(context, |create| {
            create.set_application_commands(GlobalCommands::application_commands().collect());
            create
        })
        .await
        {
            Err(err) => {
                error!(?err, "error registering global application commands");
                return Err(HandlerError::CommandSetup);
            }
            Ok(commands) => commands,
        };

        info!(
            commands = ?global_commands.iter().map(|c| &c.name).collect::<Vec<_>>(),
            "registered global commands"
        );
        if let Err(err) = self
            .save_command_ids::<GlobalCommands>(context, global_commands.into_iter())
            .await
        {
            error!(?err, "error saving global application command data");
            return Err(HandlerError::CommandSetup);
        }

        Ok(())
    }

    pub async fn setup_guild_commands(
        &self,
        context: &Context,
        ready: Ready,
    ) -> Result<(), HandlerError> {
        if !ready.guilds.is_empty() {
            let guild_commands = match try_join_all(ready.guilds.iter().map(|g| {
                g.id.set_application_commands(&context, |create| {
                    create
                        .set_application_commands(GuildCommands::application_commands().collect());
                    create
                })
            }))
            .await
            {
                Err(err) => {
                    error!(?err, "error registering guild application commands");
                    return Err(HandlerError::CommandSetup);
                }
                Ok(commands) => commands,
            };

            if let Some(first) = guild_commands.first() {
                info!(
                    commands = ?first.iter().map(|c| &c.name).collect::<Vec<_>>(),
                    "registered guild commands"
                );
            } else {
                error!("guilds list is not empty, but no guild commands were registered");
                return Err(HandlerError::CommandSetup);
            }
            if let Err(err) = stream::iter(guild_commands.into_iter())
                .map(Ok)
                .try_for_each(|cmds| async {
                    self.save_command_ids::<GuildCommands>(context, cmds.into_iter())
                        .await
                })
                .await
            {
                error!(?err, "error saving guild application command data");
                return Err(HandlerError::CommandSetup);
            }
        }

        Ok(())
    }
}
