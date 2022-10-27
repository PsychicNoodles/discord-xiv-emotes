use std::{mem, sync::Arc};

use async_trait::async_trait;
use futures::StreamExt;
use serenity::{
    builder::{CreateApplicationCommand, CreateInteractionResponse},
    model::prelude::{
        command::CommandType,
        interaction::{
            application_command::ApplicationCommandInteraction,
            message_component::MessageComponentInteraction, InteractionResponseType,
        },
        Message,
    },
    prelude::Context,
};
use strum::IntoEnumIterator;
use thiserror::Error;
use tracing::*;

use crate::{
    commands::AppCmd,
    db::models::{DbGender, DbLanguage, DbUser},
    util::{CreateApplicationCommandExt, LocalizedString},
    HandlerError, MessageDbData, INTERACTION_TIMEOUT,
};

pub const CONTENT: LocalizedString = LocalizedString {
    en: "Emote message settings",
    ja: "エモート設定",
};
pub const SAVE_BTN: LocalizedString = LocalizedString {
    en: "Save",
    ja: "保存",
};
pub const SETTINGS_SAVED: LocalizedString = LocalizedString {
    en: "Settings saved!",
    ja: "設定を保存しました！",
};
pub const NAME: LocalizedString = LocalizedString {
    en: "settings",
    ja: "設定",
};
pub const DESC: LocalizedString = LocalizedString {
    en: "Set personal emote message settings",
    ja: "個人エモート設定",
};

enum Ids {
    GenderSelect,
    LanguageSelect,
    Submit,
}

impl From<Ids> for &'static str {
    fn from(ids: Ids) -> Self {
        From::<&Ids>::from(&ids)
    }
}

impl From<&Ids> for &'static str {
    fn from(ids: &Ids) -> Self {
        match ids {
            Ids::GenderSelect => "gender_select",
            Ids::LanguageSelect => "language_select",
            Ids::Submit => "submit",
        }
    }
}

impl ToString for Ids {
    fn to_string(&self) -> String {
        Into::<&'static str>::into(self).to_string()
    }
}

#[derive(Debug, Clone, Error)]
#[error("Unrecognized component id ({0})")]
struct InvalidComponentId(String);

impl TryFrom<&str> for Ids {
    type Error = InvalidComponentId;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "gender_select" => Ok(Ids::GenderSelect),
            "language_select" => Ok(Ids::LanguageSelect),
            "submit" => Ok(Ids::Submit),
            s => Err(InvalidComponentId(s.to_string())),
        }
    }
}

#[instrument(skip(context))]
async fn handle_interaction(
    context: &Context,
    msg: &Message,
    interaction: Arc<MessageComponentInteraction>,
    user: &mut DbUser,
) -> Result<Option<DbUser>, HandlerError> {
    trace!("incoming interactions: {:?}", interaction);
    match Ids::try_from(interaction.data.custom_id.as_str()) {
        Ok(Ids::GenderSelect) => {
            let value = &interaction.data.values[0];
            let value = if let Ok(v) = value.parse() {
                v
            } else {
                error!("unexpected gender selected (not numeric): {}", value);
                return Err(HandlerError::UnexpectedData);
            };
            let gender = match DbGender::from_repr(value) {
                Some(g) => g,
                None => {
                    error!("unexpected gender selected (invalid number): {}", value);
                    return Err(HandlerError::UnexpectedData);
                }
            };
            debug!("gender selected: {:?}", gender);
            user.gender = gender;
        }
        Ok(Ids::LanguageSelect) => {
            let value = &interaction.data.values[0];
            let value = if let Ok(v) = value.parse() {
                v
            } else {
                error!("unexpected language selected (not numeric): {}", value);
                return Err(HandlerError::UnexpectedData);
            };
            let lang = match DbLanguage::from_repr(value) {
                Some(g) => g,
                None => {
                    error!("unexpected language selected (invalid number): {}", value);
                    return Err(HandlerError::UnexpectedData);
                }
            };
            debug!("language selected: {:?}", lang);
            user.language = lang;
        }
        Ok(Ids::Submit) => {
            interaction
                .create_interaction_response(context, |res| {
                    res.kind(InteractionResponseType::UpdateMessage)
                        .interaction_response_data(|d| {
                            d.content(SETTINGS_SAVED.for_user(user))
                                .components(|cmp| cmp)
                        })
                })
                .await?;
            return Ok(Some(mem::take(user)));
        }
        Err(e) => {
            error!("unexpected component id: {}", e);
        }
    }

    interaction
        .create_interaction_response(context, |res| {
            create_response(res, InteractionResponseType::UpdateMessage, user)
        })
        .await?;

    Ok(None)
}

async fn handle_interactions(
    context: &Context,
    msg: &Message,
    mut user: DbUser,
) -> Result<DbUser, HandlerError> {
    while let Some(interaction) = msg
        .await_component_interactions(context)
        .collect_limit(20)
        .timeout(INTERACTION_TIMEOUT)
        .build()
        .next()
        .await
    {
        if let Some(res) = handle_interaction(context, msg, interaction, &mut user).await? {
            return Ok(res);
        }
    }
    Err(HandlerError::TimeoutOrOverLimit)
}

#[instrument(skip(res))]
fn create_response<'a, 'b>(
    res: &'a mut CreateInteractionResponse<'b>,
    kind: InteractionResponseType,
    user: &DbUser,
) -> &'a mut CreateInteractionResponse<'b> {
    res.kind(kind).interaction_response_data(|data| {
        data.ephemeral(true)
            .content(CONTENT.for_user(user))
            .components(|c| {
                c.create_action_row(|row| {
                    row.create_select_menu(|menu| {
                        menu.custom_id(Ids::GenderSelect).options(|opts| {
                            DbGender::iter().for_each(|gender| {
                                opts.create_option(|o| {
                                    o.label(gender.for_user(user))
                                        .value(gender as i32)
                                        .default_selection(user.gender == gender)
                                });
                            });
                            opts
                        })
                    })
                });
                c.create_action_row(|row| {
                    row.create_select_menu(|menu| {
                        menu.custom_id(Ids::LanguageSelect).options(|opts| {
                            DbLanguage::iter().for_each(|lang| {
                                opts.create_option(|o| {
                                    o.label(lang.for_user(user))
                                        .value(lang as i32)
                                        .default_selection(user.language == lang)
                                });
                            });
                            opts
                        })
                    })
                });
                c.create_action_row(|row| {
                    row.create_button(|btn| {
                        btn.custom_id(Ids::Submit).label(SAVE_BTN.for_user(user))
                    })
                })
            })
    })
}

pub struct UserSettingsCmd;

#[async_trait]
impl AppCmd for UserSettingsCmd {
    fn to_application_command() -> CreateApplicationCommand
    where
        Self: Sized,
    {
        let mut cmd = CreateApplicationCommand::default();
        cmd.localized_name(NAME)
            .kind(CommandType::ChatInput)
            .localized_desc(DESC)
            .dm_permission(true);
        cmd
    }

    #[instrument(skip(handler, context))]
    async fn handle(
        cmd: &ApplicationCommandInteraction,
        handler: &crate::Handler,
        context: &Context,
        message_db_data: &MessageDbData,
    ) -> Result<(), HandlerError>
    where
        Self: Sized,
    {
        trace!("finding existing user");
        let user = message_db_data.determine_user_settings().await?;
        let user_id = cmd.user.id;

        cmd.create_interaction_response(context, |res| {
            create_response(
                res,
                InteractionResponseType::ChannelMessageWithSource,
                &user,
            )
        })
        .await?;
        let msg = cmd.get_interaction_response(context).await?;
        trace!("awaiting interactions");
        let user = handle_interactions(context, &msg, user.into_owned()).await?;

        handler
            .db
            .upsert_user(user_id.to_string(), user.language, user.gender)
            .await?;

        Ok(())
    }

    fn name() -> LocalizedString {
        NAME
    }
}
