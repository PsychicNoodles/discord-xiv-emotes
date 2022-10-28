use std::str::FromStr;

use async_trait::async_trait;
use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::interaction::application_command::ApplicationCommandInteraction,
    prelude::Context,
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
pub trait CommandsEnum: FromStr {
    async fn handle(
        self,
        cmd: &ApplicationCommandInteraction,
        handler: &Handler,
        context: &Context,
        message_db_data: &MessageDbData,
    ) -> Result<(), HandlerError>;
}

fn check_is_app_command_cap_err(err: serenity::Error) -> HandlerError {
    if let serenity::Error::Http(boxed_err) = &err {
        match **boxed_err {
            serenity::http::HttpError::UnsuccessfulRequest(
                serenity::http::error::ErrorResponse {
                    error: serenity::http::error::DiscordJsonError { code, .. },
                    ..
                },
            ) if code == 30032 => return HandlerError::ApplicationCommandCap,
            _ => {}
        };
    }
    HandlerError::Send(err)
}
