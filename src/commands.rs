use std::str::FromStr;

use async_trait::async_trait;
use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::interaction::application_command::ApplicationCommandInteraction,
    prelude::Context,
};

use crate::{Handler, HandlerError};

pub mod global;
pub mod guild;

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

#[async_trait]
pub trait CommandsEnum: FromStr {
    async fn handle(
        self,
        cmd: &ApplicationCommandInteraction,
        handler: &Handler,
        context: &Context,
    ) -> Result<(), HandlerError>;
}
