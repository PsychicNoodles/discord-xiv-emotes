use std::borrow::Cow;

use async_trait::async_trait;
use const_format::concatcp;
use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::{
        command::{CommandOptionType, CommandType},
        interaction::application_command::{ApplicationCommandInteraction, CommandDataOptionValue},
    },
    prelude::Context,
};
use tracing::*;

use crate::{
    commands::AppCmd,
    util::{CreateApplicationCommandExt, CreateApplicationCommandOptionExt, LocalizedString},
    Handler, HandlerError, MessageDbData,
};

use super::list_emotes::NAME as LIST_EMOTES_NAME;

pub const NAME: LocalizedString = LocalizedString {
    en: "emote",
    ja: "エモート",
};
pub const DESC: LocalizedString = LocalizedString {
    en: "Send an emote with an optional target user",
    ja: "エモートを選択してターゲットを任意選択して送信",
};
pub const EMOTE_OPTION_NAME: LocalizedString = LocalizedString {
    en: "emote",
    ja: "エモート",
};
pub const EMOTE_OPTION_DESC: LocalizedString = LocalizedString {
    en: "Which emote to send",
    ja: "エモートの指定",
};
pub const TARGET_OPTION_NAME: LocalizedString = LocalizedString {
    en: "target",
    ja: "ターゲット",
};
pub const TARGET_OPTION_DESC: LocalizedString = LocalizedString {
    en: "Who to target with the emote (can be a mention)",
    ja: "エモートのターゲット（メンション可）",
};
pub const EMOTE_NOT_EXISTS: LocalizedString = LocalizedString {
    en: concatcp!(
        "That's not a valid emote! Check the list of known emotes with /",
        LIST_EMOTES_NAME.en
    ),
    ja: concatcp!(
        "存在しないエモートを入力しました。エモート一覧のコマンドは/",
        LIST_EMOTES_NAME.ja
    ),
};
pub const EMOTE_SENT: LocalizedString = LocalizedString {
    en: "Emote sent!",
    ja: "送信しました！",
};

pub struct EmoteCmd;

#[async_trait]
impl AppCmd for EmoteCmd {
    fn to_application_command() -> CreateApplicationCommand
    where
        Self: Sized,
    {
        let mut cmd = CreateApplicationCommand::default();
        cmd.localized_name(NAME)
            .kind(CommandType::ChatInput)
            .localized_desc(DESC)
            .create_option(|opt| {
                opt.kind(CommandOptionType::String)
                    .localized_name(EMOTE_OPTION_NAME)
                    .localized_desc(EMOTE_OPTION_DESC)
                    .required(true)
            })
            .create_option(|opt| {
                opt.kind(CommandOptionType::String)
                    .localized_name(TARGET_OPTION_NAME)
                    .localized_desc(TARGET_OPTION_DESC)
            })
            .dm_permission(true);
        cmd
    }

    #[instrument(skip(cmd, handler, context))]
    async fn handle(
        cmd: &ApplicationCommandInteraction,
        handler: &Handler,
        context: &Context,
        message_db_data: &MessageDbData,
    ) -> Result<(), HandlerError>
    where
        Self: Sized,
    {
        let emote = &cmd
            .data
            .options
            .get(0)
            .and_then(|o| o.resolved.as_ref())
            .and_then(|v| {
                if let CommandDataOptionValue::String(s) = v {
                    Some(s)
                } else {
                    None
                }
            })
            .ok_or(HandlerError::UnexpectedData)?;

        let user_settings = message_db_data.determine_user_settings().await?;
        let guild = message_db_data.guild().await?.unwrap_or_default();

        info!(emote, "emote command");

        let emote = match emote.get(0..0) {
            None => {
                error!("emote is empty");
                return Err(HandlerError::UnrecognizedEmote("(empty)".to_string()));
            }
            Some("/") => Cow::Borrowed(emote.as_str()),
            Some(s) if s == guild.prefix => Cow::Borrowed(emote.trim_start_matches(&guild.prefix)),
            Some(_) => Cow::Owned(["/", emote].concat()),
        };
        trace!(?emote, "checking if emote exists");
        if !handler.contains_emote(&emote) {
            cmd.create_interaction_response(context, |res| {
                res.interaction_response_data(|data| {
                    data.ephemeral(true)
                        .content(EMOTE_NOT_EXISTS.for_user(&user_settings))
                })
            })
            .await?;
            return Ok(());
        }

        let emote_data = handler
            .get_emote_data(&emote)
            .ok_or_else(|| HandlerError::UnrecognizedEmote(emote.to_string()))?;
        let target = cmd
            .data
            .options
            .get(1)
            .and_then(|opt| opt.value.clone())
            .and_then(|value| value.as_str().map(ToString::to_string));
        let body = handler
            .build_emote_message(emote_data, message_db_data, &cmd.user, target.as_deref())
            .await?;
        debug!(body, resolved = ?cmd.data.resolved, "processed emote");
        cmd.channel_id
            .send_message(context, |m| m.content(body))
            .await?;
        handler
            .log_emote(
                &cmd.user.id,
                cmd.guild_id.as_ref(),
                cmd.data.resolved.users.keys(),
                emote_data,
            )
            .await?;

        cmd.create_interaction_response(context, |res| {
            res.interaction_response_data(|d| {
                d.ephemeral(true).content(format!(
                    "{} ({}{})",
                    EMOTE_SENT.for_user(&user_settings),
                    emote,
                    if let Some(t) = &target {
                        [" ".to_string(), t.to_string()].concat()
                    } else {
                        "".to_string()
                    }
                ))
            })
        })
        .await?;

        Ok(())
    }

    fn name() -> LocalizedString {
        NAME
    }
}
