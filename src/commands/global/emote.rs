use std::borrow::Cow;

use async_trait::async_trait;
use const_format::concatcp;
use serenity::{
    builder::CreateApplicationCommand,
    model::prelude::{
        command::{CommandOptionType, CommandType},
        interaction::application_command::{ApplicationCommandInteraction, CommandData},
    },
    prelude::{Context, Mentionable},
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

#[instrument(skip(context))]
fn resolve_mention(data: &CommandData, context: &Context) -> Option<String> {
    if let Some(user) = data.resolved.users.values().next() {
        debug!("resolved to user");
        Some(user.mention().to_string())
    } else if let Some(role) = data.resolved.roles.values().next() {
        debug!("resolved to role");
        Some(role.mention().to_string())
    } else if let Some(channel) = data.resolved.channels.values().next() {
        debug!("resolved to channel");
        context
            .cache
            .channel(channel.id)
            .map(|c| c.mention().to_string())
    } else if let Some(plain) = data.options.get(1).and_then(|opt| opt.value.clone()) {
        debug!("resolved to plain text");
        plain.as_str().map(ToString::to_string)
    } else {
        None
    }
}

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
        let emote = &cmd
            .data
            .options
            .get(0)
            .and_then(|o| o.value.as_ref())
            .and_then(|v| v.as_str())
            .ok_or(HandlerError::UnexpectedData)?;
        let target = resolve_mention(&cmd.data, context);
        trace!("target is {:?}", target);

        let user_settings = message_db_data.determine_user_settings().await?;
        let guild = message_db_data.guild().await?.clone().unwrap_or_default();

        let emote = match emote.get(0..0) {
            None => {
                error!("emote is empty");
                return Err(HandlerError::UnrecognizedEmote("(empty)".to_string()));
            }
            Some("/") => Cow::Borrowed(*emote),
            Some(s) if s == guild.prefix => Cow::Borrowed(emote.trim_start_matches(&guild.prefix)),
            Some(_) => Cow::Owned(["/", emote].concat()),
        };
        trace!("checking if emote exists: {:?}", emote);
        if !handler.log_message_repo.contains_emote(&emote) {
            cmd.create_interaction_response(context, |res| {
                res.interaction_response_data(|data| {
                    data.ephemeral(true)
                        .content(EMOTE_NOT_EXISTS.for_user(&user_settings))
                })
            })
            .await?;
            return Ok(());
        }

        let body = handler
            .build_emote_message(&emote, message_db_data, &cmd.user, target.as_deref())
            .await?;
        cmd.channel_id
            .send_message(context, |m| m.content(body))
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
