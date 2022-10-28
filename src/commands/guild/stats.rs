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

pub const GUILD_SUB_NAME: LocalizedString = LocalizedString {
    en: "guild",
    ja: "サーバー",
};
pub const GUILD_SUB_DESC: LocalizedString = LocalizedString {
    en: "Emote usage statistics for the current guild",
    ja: "サーバーの使用統計",
};
pub const GUILD_USER_SUB_NAME: LocalizedString = LocalizedString {
    en: "guild-user",
    ja: "サーバーのユーザー",
};
pub const GUILD_USER_SUB_DESC: LocalizedString = LocalizedString {
    en: "Emote usage statistics for a user within the current guild",
    ja: "ユーザーのサーバー内の使用統計",
};
pub const RECEIVED_GUILD_SUB_NAME: LocalizedString = LocalizedString {
    en: "guild",
    ja: "サーバー",
};
pub const RECEIVED_GUILD_SUB_DESC: LocalizedString = LocalizedString {
    en: "Emote received usage statistics for the current guild",
    ja: "サーバーの使用統計",
};
pub const RECEIVED_GUILD_USER_SUB_NAME: LocalizedString = LocalizedString {
    en: "guild-user",
    ja: "サーバーのユーザー",
};
pub const RECEIVED_GUILD_USER_SUB_DESC: LocalizedString = LocalizedString {
    en: "Emote received usage statistics for a user within the current guild",
    ja: "ユーザーのサーバー内の使用統計",
};

pub struct GuildStatsCmd;

#[async_trait]
impl AppCmd for GuildStatsCmd {
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
                    .localized_name(GUILD_SUB_NAME)
                    .localized_desc(GUILD_SUB_DESC)
            })
            .create_option(|opt| {
                opt.kind(CommandOptionType::SubCommand)
                    .localized_name(GUILD_USER_SUB_NAME)
                    .localized_desc(GUILD_USER_SUB_DESC)
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
                            .localized_name(RECEIVED_GUILD_SUB_NAME)
                            .localized_desc(RECEIVED_GUILD_SUB_DESC)
                    })
                    .create_sub_option(|grp| {
                        grp.kind(CommandOptionType::SubCommand)
                            .localized_name(RECEIVED_GUILD_USER_SUB_NAME)
                            .localized_desc(RECEIVED_GUILD_USER_SUB_DESC)
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
        let guild_id = cmd.guild_id.ok_or(HandlerError::NotGuild)?;
        let user_id_opt = cmd.data.resolved.users.keys().next();
        let top_option = cmd
            .data
            .options
            .get(0)
            .ok_or(HandlerError::UnexpectedData)?;
        let sub_option = cmd.data.options.get(1);
        let kind = match (&top_option.name, sub_option.map(|o| &o.name)) {
            (_t, _) if GUILD_SUB_NAME.any_eq(_t) => EmoteLogQuery::Guild(guild_id),
            (_t, _) if GUILD_USER_SUB_NAME.any_eq(_t) => EmoteLogQuery::GuildUser((
                guild_id,
                *user_id_opt.ok_or(HandlerError::UnexpectedData)?,
            )),
            (_t, Some(_s))
                if RECEIVED_GROUP_NAME.any_eq(_t) && RECEIVED_GUILD_SUB_NAME.any_eq(_s) =>
            {
                EmoteLogQuery::ReceivedGuild(guild_id)
            }
            (_t, Some(_s))
                if RECEIVED_GROUP_NAME.any_eq(_t) && RECEIVED_GUILD_USER_SUB_NAME.any_eq(_s) =>
            {
                EmoteLogQuery::ReceivedGuildUser((
                    guild_id,
                    *user_id_opt.ok_or(HandlerError::UnexpectedData)?,
                ))
            }
            _ => {
                error!("could not parse guild stat kind");
                return Err(HandlerError::UnexpectedData);
            }
        };
        debug!("guild stat kind: {:?}", kind);

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