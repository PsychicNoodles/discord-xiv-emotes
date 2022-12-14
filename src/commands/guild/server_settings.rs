use std::{mem, sync::Arc};

use async_trait::async_trait;
use futures::StreamExt;
use serenity::{
    builder::{CreateApplicationCommand, CreateInteractionResponse},
    model::{
        prelude::{
            command::CommandType,
            component::{ActionRowComponent, InputTextStyle},
            interaction::{
                application_command::ApplicationCommandInteraction,
                message_component::MessageComponentInteraction, InteractionResponseType,
            },
            Message,
        },
        Permissions,
    },
    prelude::Context,
};
use strum::IntoEnumIterator;
use thiserror::Error;
use tracing::*;

use crate::{
    commands::AppCmd,
    db::models::{DbGender, DbGuild, DbLanguage, DbUser},
    util::{CreateApplicationCommandExt, LocalizedString},
    Handler, HandlerError, MessageDbData, INTERACTION_TIMEOUT,
};

pub const CONTENT: LocalizedString = LocalizedString {
    en: "Server-wide emote message settings",
    ja: "サーバーのエモート設定",
};
pub const PREFIX_INPUT_BTN: LocalizedString = LocalizedString {
    en: "Input a command prefix, currently: ",
    ja: "コマンドプレフィックスを入力、現在：",
};
pub const PREFIX_INPUT_MODAL_CONTENT: LocalizedString = LocalizedString {
    en: "Input a command prefix (up to 5 characters)",
    ja: "コマンドプレフィックスを入力してください（5文字まで）",
};
pub const PREFIX_INPUT_MODAL_INPUT: LocalizedString = LocalizedString {
    en: "Command prefix",
    ja: "コマンドプレフィックス",
};
pub const PREFIX_INPUT_MODAL_TITLE: LocalizedString = LocalizedString {
    en: "Server-wide command prefix",
    ja: "サーバーのコマンドプレフィックス",
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
    en: "server-settings",
    ja: "サーバー設定",
};
pub const DESC: LocalizedString = LocalizedString {
    en: "Set the default emote message settings used for this server",
    ja: "このサーバーのデフォルトのエモート設定",
};

const PREFIX_INPUT_MODAL: &str = "prefix_input_modal";
const PREFIX_INPUT_MODAL_BTN: &str = "prefix_input_modal_btn";

enum Ids {
    GenderSelect,
    LanguageSelect,
    PrefixInputBtn,
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
            Ids::PrefixInputBtn => "prefix_input_btn",
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
            "prefix_input_btn" => Ok(Ids::PrefixInputBtn),
            "submit" => Ok(Ids::Submit),
            s => Err(InvalidComponentId(s.to_string())),
        }
    }
}

#[instrument(skip(context))]
async fn handle_interaction(
    context: &Context,
    msg: &Message,
    user: &DbUser,
    interaction: Arc<MessageComponentInteraction>,
    guild: &mut DbGuild,
) -> Result<Option<DbGuild>, HandlerError> {
    match Ids::try_from(interaction.data.custom_id.as_str()) {
        Ok(Ids::GenderSelect) => {
            let value = &interaction.data.values[0];
            let value = if let Ok(v) = value.parse() {
                v
            } else {
                error!(value, "unexpected gender selected (not numeric)");
                return Err(HandlerError::UnexpectedData);
            };
            let gender = match DbGender::from_repr(value) {
                Some(g) => g,
                None => {
                    error!(value, "unexpected gender selected (invalid number)");
                    return Err(HandlerError::UnexpectedData);
                }
            };
            debug!(?gender, "gender selected");
            guild.gender = gender;
        }
        Ok(Ids::LanguageSelect) => {
            let value = &interaction.data.values[0];
            let value = if let Ok(v) = value.parse() {
                v
            } else {
                error!(value, "unexpected language selected (not numeric)");
                return Err(HandlerError::UnexpectedData);
            };
            let lang = match DbLanguage::from_repr(value) {
                Some(g) => g,
                None => {
                    error!(value, "unexpected language selected (invalid number)");
                    return Err(HandlerError::UnexpectedData);
                }
            };
            debug!(?lang, "language selected");
            guild.language = lang;
        }
        Ok(Ids::PrefixInputBtn) => {
            debug!("prefix input");
            let span = debug_span!("prefix_input_modal_interaction");
            async move {
                interaction
                    .create_interaction_response(context, |res| {
                        res.kind(InteractionResponseType::Modal)
                            .interaction_response_data(|d| {
                                d.content(PREFIX_INPUT_MODAL_CONTENT.for_user(user))
                                    .components(|c| {
                                        c.create_action_row(|row| {
                                            row.create_input_text(|inp| {
                                                inp.custom_id(PREFIX_INPUT_MODAL)
                                                    .style(InputTextStyle::Short)
                                                    .label(PREFIX_INPUT_MODAL_INPUT.for_user(user))
                                                    .max_length(5)
                                            })
                                        })
                                    })
                                    .title(PREFIX_INPUT_MODAL_TITLE.for_user(user))
                                    .custom_id(PREFIX_INPUT_MODAL_BTN)
                            })
                    })
                    .await?;

                if let Some(modal_interaction) = msg
                    .await_modal_interaction(context)
                    .timeout(INTERACTION_TIMEOUT)
                    .await
                {
                    match &modal_interaction.data.components[0].components[0] {
                        ActionRowComponent::InputText(cmp) => {
                            trace!(prefix = cmp.value, "setting prefix");
                            guild.prefix = cmp.value.clone();
                            modal_interaction
                                .create_interaction_response(context, |res| {
                                    create_response(
                                        res,
                                        InteractionResponseType::UpdateMessage,
                                        user,
                                        guild,
                                    )
                                })
                                .await?;
                        }
                        cmp => {
                            error!(?cmp, "modal component was not an input text");
                            return Err(HandlerError::UnexpectedData);
                        }
                    }
                }
                Ok(())
            }
            .instrument(span)
            .await?;
            // don't send typical interaction response
            return Ok(None);
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
            return Ok(Some(mem::take(guild)));
        }
        Err(err) => {
            error!(?err, "unexpected component id");
        }
    }

    interaction
        .create_interaction_response(context, |res| {
            create_response(res, InteractionResponseType::UpdateMessage, user, guild)
        })
        .await?;

    Ok(None)
}

async fn handle_interactions(
    context: &Context,
    msg: &Message,
    user: &DbUser,
    mut db_guild: DbGuild,
) -> Result<DbGuild, HandlerError> {
    while let Some(interaction) = msg
        .await_component_interactions(context)
        .collect_limit(20)
        .timeout(INTERACTION_TIMEOUT)
        .build()
        .next()
        .await
    {
        if let Some(res) =
            handle_interaction(context, msg, user, interaction, &mut db_guild).await?
        {
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
    db_guild: &DbGuild,
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
                                        .default_selection(db_guild.gender == gender)
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
                                        .default_selection(db_guild.language == lang)
                                });
                            });
                            opts
                        })
                    })
                });
                c.create_action_row(|row| {
                    row.create_button(|btn| {
                        btn.custom_id(Ids::PrefixInputBtn)
                            .label([PREFIX_INPUT_BTN.for_user(user), &db_guild.prefix].concat())
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

pub struct ServerSettingsCmd;

#[async_trait]
impl AppCmd for ServerSettingsCmd {
    fn to_application_command() -> CreateApplicationCommand
    where
        Self: Sized,
    {
        let mut cmd = CreateApplicationCommand::default();
        cmd.localized_name(NAME)
            .kind(CommandType::ChatInput)
            .localized_desc(DESC)
            .default_member_permissions(Permissions::MANAGE_CHANNELS);
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
        let user = message_db_data.determine_user_settings().await?;
        let guild = message_db_data.guild().await?.unwrap_or_default();
        let guild_id = cmd.guild_id.ok_or(HandlerError::NotGuild)?;
        info!(?guild_id, "server settings command");

        cmd.create_interaction_response(context, |res| {
            create_response(
                res,
                InteractionResponseType::ChannelMessageWithSource,
                &user,
                &guild,
            )
        })
        .await?;
        let msg = cmd.get_interaction_response(context).await?;
        trace!("awaiting interactions");
        let guild = handle_interactions(context, &msg, &user, guild.into_owned()).await?;

        handler
            .db
            .upsert_guild(&guild_id, guild.language, guild.gender, guild.prefix)
            .await?;

        Ok(())
    }

    fn name() -> LocalizedString {
        NAME
    }
}
