use async_trait::async_trait;
use futures::StreamExt;
use log::*;
use serenity::{
    builder::{CreateApplicationCommand, CreateInteractionResponse},
    model::prelude::{
        command::CommandType,
        component::{ActionRowComponent, InputTextStyle},
        interaction::{
            application_command::ApplicationCommandInteraction, InteractionResponseType,
        },
        Message,
    },
    prelude::Context,
};
use strum::IntoEnumIterator;
use thiserror::Error;

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
                db_guild.gender = gender;
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
                db_guild.language = lang;
            }
            Ok(Ids::PrefixInputBtn) => {
                debug!("prefix input");
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
                                                    .label("Target name")
                                                    .max_length(5)
                                            })
                                        })
                                    })
                                    .title("Custom target input")
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
                            trace!("setting prefix to: {}", cmp.value);
                            db_guild.prefix = cmp.value.clone();
                            modal_interaction
                                .create_interaction_response(context, |res| {
                                    create_response(
                                        res,
                                        InteractionResponseType::UpdateMessage,
                                        user,
                                        &db_guild,
                                    )
                                })
                                .await?;
                        }
                        cmp => {
                            error!("modal component was not an input text: {:?}", cmp);
                            return Err(HandlerError::UnexpectedData);
                        }
                    }
                }
                // don't send typical interaction response
                continue;
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
                return Ok(db_guild);
            }
            Err(e) => {
                error!("unexpected component id: {}", e);
            }
        }

        interaction
            .create_interaction_response(context, |res| {
                create_response(res, InteractionResponseType::UpdateMessage, user, &db_guild)
            })
            .await?;
    }
    Err(HandlerError::TimeoutOrOverLimit)
}

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
            .localized_desc(DESC);
        cmd
    }

    async fn handle(
        cmd: &ApplicationCommandInteraction,
        handler: &Handler,
        context: &Context,
        message_db_data: &MessageDbData,
    ) -> Result<(), HandlerError>
    where
        Self: Sized,
    {
        trace!("finding existing guild");
        let user = message_db_data.determine_user_settings().await?;
        let guild = message_db_data.guild().await?.clone().unwrap_or_default();

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
        let guild = handle_interactions(context, &msg, &user, guild).await?;

        handler
            .db
            .upsert_guild(guild.discord_id, guild.language, guild.gender, guild.prefix)
            .await?;

        Ok(())
    }

    fn name() -> LocalizedString {
        NAME
    }
}
