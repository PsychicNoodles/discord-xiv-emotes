use serenity::{
    model::prelude::{GuildId, Mention, Message, UserId},
    prelude::{Context, Mentionable},
    utils::MessageBuilder,
};
use std::{borrow::Cow, fmt::Debug, sync::Arc};
use tracing::*;
use xiv_emote_parser::log_message::{
    condition::{Character, DynamicText, Gender},
    parser::Text,
    LogMessageAnswers,
};

use crate::{db::models::DbUser, MessageDbData};

use super::{EmoteData, Handler, HandlerError};

// untargeted messages shouldn't reference target character at all, but just in case
pub const UNTARGETED_TARGET: Character =
    Character::new("Godbert Manderville", Gender::Male, false, false);

impl Handler {
    pub fn emote_list_by_id(&self) -> impl Iterator<Item = &String> {
        let mut values: Vec<_> = self.emotes.iter().collect();
        values.sort_unstable_by(|(_, v1), (_, v2)| v1.id.cmp(&v2.id));
        values.into_iter().map(|(k, _)| k)
    }

    pub async fn upsert_emotes(&self) -> Result<(), HandlerError> {
        self.db
            .upsert_emotes(
                self.emotes
                    .values()
                    .map(|data| (data.id as i32, data.name.clone())),
            )
            .await?;
        Ok(())
    }

    pub fn contains_emote(&self, emote: &str) -> bool {
        self.emotes.contains_key(emote)
    }

    pub fn get_emote_data(&self, emote: &str) -> Option<&Arc<EmoteData>> {
        self.emotes.get(emote)
    }

    #[instrument(skip(self, context, msg))]
    pub async fn process_message_input<'a>(
        &self,
        context: &Context,
        mparts: &[&str],
        msg: &Message,
        message_db_data: &MessageDbData<'a>,
    ) -> Result<(), HandlerError> {
        let (original_emote, mention) = mparts.split_first().ok_or(HandlerError::EmptyCommand)?;
        let emote = ["/", original_emote].concat();
        let mention = if mention.is_empty() {
            None
        } else {
            Some(mention.join(" "))
        };

        debug!(emote, ?mention, "parsed message");

        let emote = self.get_emote_data(&emote);

        match (emote, mention) {
            (Some(emote), mention_opt) => {
                let body = self
                    .build_emote_message(
                        emote,
                        message_db_data,
                        &msg.author,
                        mention_opt.as_ref().map(AsRef::as_ref),
                    )
                    .await?;
                debug!(body, "emote result");
                msg.reply(context, body).await?;
                self.log_emote(
                    &msg.author.id,
                    msg.guild_id.as_ref(),
                    msg.mentions.iter().map(|u| &u.id),
                    emote,
                )
                .await?;
                Ok(())
            }
            (_, _) => {
                warn!("could not find matching emote");
                Err(HandlerError::UnrecognizedEmote(original_emote.to_string()))
            }
        }
    }

    #[instrument(skip(self))]
    pub async fn build_emote_message<'a, T: Mentionable + Debug>(
        &self,
        emote: &Arc<EmoteData>,
        message_db_data: &MessageDbData<'a>,
        author_mentionable: &T,
        target: Option<&str>,
    ) -> Result<String, HandlerError> {
        enum BuilderAction<'a> {
            Mention(Mention),
            Text(Cow<'a, str>),
        }

        impl<'a> BuilderAction<'a> {
            fn do_action(self, msg_builder: &mut MessageBuilder) {
                match self {
                    BuilderAction::Mention(m) => msg_builder.mention(&m),
                    BuilderAction::Text(s) => msg_builder.push(s),
                };
            }
        }

        let author_mention = author_mentionable.mention();

        let user = message_db_data.determine_user_settings().await?;
        let DbUser {
            language, gender, ..
        } = user.as_ref();

        let localized_messages = language.with_emote_data(emote);
        let condition_texts = if target.is_some() {
            localized_messages.targeted.clone()
        } else {
            localized_messages.untargeted.clone()
        };

        let origin_char = Character::new_from_string(
            author_mention.mention().to_string(),
            gender.into(),
            true,
            false,
        );
        let target_char = target
            .as_ref()
            .map(|t| Character::new_from_string(t.to_string(), Gender::Male, true, false))
            .unwrap_or(UNTARGETED_TARGET);
        debug!(emote.name, ?origin_char, ?target_char, "building emote");
        let answers = LogMessageAnswers::new(origin_char, target_char)?;

        Ok(condition_texts
            .into_map_texts(&answers, move |text| match text {
                Text::Dynamic(d) => match d {
                    DynamicText::NpcOriginName
                    | DynamicText::PlayerOriginNameEn
                    | DynamicText::PlayerOriginNameJp => Ok(BuilderAction::Mention(author_mention)),
                    DynamicText::NpcTargetName
                    | DynamicText::PlayerTargetNameEn
                    | DynamicText::PlayerTargetNameJp => match &target {
                        Some(t) => Ok(BuilderAction::Text(Cow::Borrowed(t))),
                        None => Err(HandlerError::TargetNone),
                    },
                },
                Text::Static(s) => Ok(BuilderAction::Text(Cow::Owned(s))),
            })
            .fold(Ok(MessageBuilder::new()), |builder_res, action_res| match (
                builder_res,
                action_res,
            ) {
                (Err(e), _) | (_, Err(e)) => Err(e),
                (Ok(mut builder), Ok(action)) => {
                    action.do_action(&mut builder);
                    Ok(builder)
                }
            })?
            .build())
    }

    #[instrument(skip(self))]
    pub async fn log_emote(
        &self,
        user_discord_id: &UserId,
        guild_discord_id: Option<&GuildId>,
        target_discord_ids: impl Iterator<Item = &UserId> + Debug,
        messages: &Arc<EmoteData>,
    ) -> Result<(), HandlerError> {
        if let Ok(id) = messages.id.try_into() {
            self.db
                .insert_emote_log(user_discord_id, guild_discord_id, target_discord_ids, id)
                .await?;
        } else {
            error!(messages.id, "could not convert emote id to i32");
        };
        Ok(())
    }
}
