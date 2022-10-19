use log::*;
use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::{
        command::CommandType, interaction::application_command::ApplicationCommandInteraction,
    },
    prelude::Context,
};

use crate::{
    db::{Db, DbUser},
    HandlerError,
};

use super::Commands;

pub async fn handle_chat_input(
    cmd: &ApplicationCommandInteraction,
    db: &Db,
    context: &Context,
) -> Result<(), HandlerError> {
    trace!("finding existing user");
    let discord_id = cmd.user.id.to_string();
    let user = db.find_user(discord_id.clone()).await?.unwrap_or(DbUser {
        discord_id,
        ..Default::default()
    });
    todo!()
}

pub fn register_chat_input(cmd: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    cmd.name(Commands::UserSettings)
        .kind(CommandType::ChatInput)
        .description("Change personal chat message settings")
        .dm_permission(true)
}
