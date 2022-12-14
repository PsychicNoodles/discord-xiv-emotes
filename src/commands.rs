use std::{collections::HashMap, fmt::Debug, hash::Hash, str::FromStr};

use async_trait::async_trait;
use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::{interaction::application_command::ApplicationCommandInteraction, CommandId},
    prelude::{Context, TypeMapKey},
};

use crate::{util::LocalizedString, Handler, HandlerError, MessageDbData};

pub mod global;
pub mod guild;
pub mod stats;

#[async_trait]
trait AppCmd {
    fn to_application_command() -> CreateApplicationCommand
    where
        Self: Sized;
    async fn handle(
        cmd: &ApplicationCommandInteraction,
        handler: &Handler,
        context: &Context,
        message_db_data: &MessageDbData,
    ) -> Result<(), HandlerError>
    where
        Self: Sized;
    fn name() -> LocalizedString;
}

#[async_trait]
pub trait CommandsEnum:
    FromStr + TypeMapKey<Value = HashMap<CommandId, Self>> + Debug + Copy + Eq + Hash
{
    async fn handle(
        self,
        cmd: &ApplicationCommandInteraction,
        handler: &Handler,
        context: &Context,
        message_db_data: &MessageDbData,
    ) -> Result<(), HandlerError>;
}
