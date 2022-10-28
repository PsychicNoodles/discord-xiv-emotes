use async_trait::async_trait;
use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::{
        command::{CommandOptionType, CommandType},
        interaction::application_command::ApplicationCommandInteraction,
    },
    prelude::Context,
};
use tracing::*;

use crate::{
    commands::{stats::*, AppCmd},
    util::{CreateApplicationCommandExt, CreateApplicationCommandOptionExt, LocalizedString},
    Handler, HandlerError, MessageDbData,
};

pub const USER_SUB_NAME: LocalizedString = LocalizedString {
    en: "user",
    ja: "ユーザー",
};
pub const USER_SUB_DESC: LocalizedString = LocalizedString {
    en: "Global emote usage statistics for a user",
    ja: "ユーザーの全体使用統計",
};
pub const RECEIVED_USER_SUB_NAME: LocalizedString = LocalizedString {
    en: "user",
    ja: "ユーザー",
};
pub const RECEIVED_USER_SUB_DESC: LocalizedString = LocalizedString {
    en: "Global emote received usage statistics for a user",
    ja: "ユーザーの全体使用統計",
};

pub struct GlobalStatsCmd;

#[async_trait]
impl AppCmd for GlobalStatsCmd {
    fn to_application_command() -> CreateApplicationCommand
    where
        Self: Sized,
    {
        let mut cmd = CreateApplicationCommand::default();
        cmd.localized_name(NAME)
            .kind(CommandType::ChatInput)
            .localized_desc(DESC)
            .create_option(|opt| {
                opt.kind(CommandOptionType::SubCommand)
                    .localized_name(USER_SUB_NAME)
                    .localized_desc(USER_SUB_DESC)
                    .create_sub_option(|sub| {
                        sub.kind(CommandOptionType::User)
                            .localized_name(USER_OPT_NAME)
                            .localized_desc(USER_OPT_DESC)
                            .required(true)
                    })
            })
            .create_option(|opt| {
                opt.kind(CommandOptionType::SubCommandGroup)
                    .localized_name(RECEIVED_GROUP_NAME)
                    .localized_desc(RECEIVED_GROUP_DESC)
                    .create_sub_option(|grp| {
                        grp.kind(CommandOptionType::SubCommand)
                            .localized_name(RECEIVED_USER_SUB_NAME)
                            .localized_desc(RECEIVED_USER_SUB_DESC)
                            .create_sub_option(|sub| {
                                sub.kind(CommandOptionType::User)
                                    .localized_name(USER_OPT_NAME)
                                    .localized_desc(USER_OPT_DESC)
                                    .required(true)
                            })
                    })
            });
        cmd
    }

    #[instrument(skip(handler, context))]
    async fn handle(
        cmd: &ApplicationCommandInteraction,
        handler: &Handler,
        context: &Context,
        message_db_data: &MessageDbData,
    ) -> Result<(), HandlerError>
    where
        Self: Sized,
    {
        let user = message_db_data.user().await?.unwrap_or_default();
        let user_id_opt = cmd.data.resolved.users.keys().next();
        let top_option = cmd
            .data
            .options
            .get(0)
            .ok_or(HandlerError::UnexpectedData)?;
        let sub_option = top_option.options.get(0);
        let kind = match (&top_option.name, sub_option.map(|o| &o.name)) {
            (_s, _) if USER_SUB_NAME.any_eq(_s) => {
                EmoteLogQuery::User(*user_id_opt.ok_or(HandlerError::UnexpectedData)?)
            }
            (_s, Some(_t))
                if RECEIVED_GROUP_NAME.any_eq(_s) && RECEIVED_USER_SUB_NAME.any_eq(_t) =>
            {
                EmoteLogQuery::ReceivedUser(*user_id_opt.ok_or(HandlerError::UnexpectedData)?)
            }
            _ => {
                error!("could not parse global stat kind");
                return Err(HandlerError::UnexpectedData);
            }
        };
        debug!("global stat kind: {:?}", kind);

        let count = handler.db.fetch_emote_log_count(&kind).await?;
        let message = kind.to_message(count, &user);
        cmd.create_interaction_response(context, |res| {
            res.interaction_response_data(|d| d.content(message))
        })
        .await?;

        Ok(())
    }

    fn name() -> LocalizedString {
        return NAME;
    }
}
