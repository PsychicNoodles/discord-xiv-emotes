use futures::StreamExt;
use log::*;
use serenity::{
    builder::{CreateApplicationCommand, CreateComponents, CreateInteractionResponse},
    model::{
        prelude::{
            command::CommandType,
            interaction::{
                application_command::ApplicationCommandInteraction, InteractionResponseType,
            },
            Message,
        },
        user::User,
    },
    prelude::Context,
};
use strum::IntoEnumIterator;
use thiserror::Error;

use crate::{
    db::{Db, DbUser, DbUserGender, DbUserLanguage},
    HandlerError, INTERACTION_TIMEOUT,
};

use super::Commands;

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

async fn handle_interactions(
    context: &Context,
    msg: &Message,
    author: &User,
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
                let gender = match DbUserGender::from_repr(value) {
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
                let lang = match DbUserLanguage::from_repr(value) {
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
                                d.content("Settings saved!").components(|cmp| cmp)
                            })
                    })
                    .await?;
                return Ok(user);
            }
            Err(e) => {
                error!("unexpected component id: {}", e);
            }
        }

        interaction
            .create_interaction_response(context, |res| {
                create_response(res, InteractionResponseType::UpdateMessage, author, &user)
            })
            .await?;
    }
    Err(HandlerError::TimeoutOrOverLimit)
}

fn create_response<'a, 'b>(
    res: &'a mut CreateInteractionResponse<'b>,
    kind: InteractionResponseType,
    author: &User,
    user: &DbUser,
) -> &'a mut CreateInteractionResponse<'b> {
    res.kind(kind).interaction_response_data(|data| {
        data.ephemeral(true)
            .content(format!("User settings for {}", author.name))
            .components(|c| create_user_settings_components(c, user))
    })
}

fn create_user_settings_components<'a>(
    create_components: &'a mut CreateComponents,
    user: &DbUser,
) -> &'a mut CreateComponents {
    create_components.create_action_row(|row| {
        row.create_select_menu(|menu| {
            menu.custom_id(Ids::GenderSelect).options(|opts| {
                DbUserGender::iter().for_each(|gender| {
                    opts.create_option(|o| {
                        o.label(gender.to_string(user.language))
                            .value(gender as i32)
                            .default_selection(user.gender == gender)
                    });
                });
                opts
            })
        })
    });
    create_components.create_action_row(|row| {
        row.create_select_menu(|menu| {
            menu.custom_id(Ids::LanguageSelect).options(|opts| {
                DbUserLanguage::iter().for_each(|lang| {
                    opts.create_option(|o| {
                        o.label(lang.to_string(user.language))
                            .value(lang as i32)
                            .default_selection(user.language == lang)
                    });
                });
                opts
            })
        })
    });
    create_components.create_action_row(|row| {
        row.create_button(|btn| {
            btn.custom_id(Ids::Submit);
            btn.label("Save")
        })
    })
}

pub async fn handle_chat_input(
    cmd: &ApplicationCommandInteraction,
    db: &Db,
    context: &Context,
) -> Result<(), HandlerError> {
    trace!("finding existing user");
    let discord_id = cmd.user.id.to_string();
    let user = db.find_user(discord_id.clone()).await?.unwrap_or(DbUser {
        discord_id,
        ..Default::default()
    });

    cmd.create_interaction_response(context, |res| {
        create_response(
            res,
            InteractionResponseType::ChannelMessageWithSource,
            &cmd.user,
            &user,
        )
    })
    .await?;
    let msg = cmd.get_interaction_response(context).await?;
    trace!("awaiting interactions");
    let user = handle_interactions(context, &msg, &cmd.user, user).await?;

    db.upsert_user(user.discord_id, user.language, user.gender)
        .await?;

    Ok(())
}

pub fn register_chat_input(cmd: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    cmd.name(Commands::UserSettings)
        .kind(CommandType::ChatInput)
        .description("Change personal chat message settings")
        .dm_permission(true)
}
