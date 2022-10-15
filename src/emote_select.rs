use std::ops::Deref;

use futures::stream::StreamExt;
use log::*;
use serenity::{
    builder::{CreateApplicationCommand, CreateComponents},
    model::{
        guild::Member,
        id::UserId,
        prelude::{
            command::CommandType,
            component::InputTextStyle,
            interaction::{
                application_command::ApplicationCommandInteraction,
                message_component::MessageComponentInteraction, InteractionResponseType,
            },
            Message,
        },
    },
    prelude::Context,
};
use xiv_emote_parser::{
    log_message::{
        condition::{Character, Gender},
        parser::extract_condition_texts,
        LogMessageAnswers,
    },
    repository::LogMessageRepository,
};

use crate::{send_emote, HandlerError, Target, INTERACTION_TIMEOUT, UNTARGETED_TARGET};

pub const CHAT_INPUT_COMMAND_NAME: &'static str = "emote";

const TARGET_SELECT_ID: &'static str = "user_select";
const SWITCH_TO_INPUT_ID: &'static str = "switch_to_input";
const TARGET_INPUT_ID: &'static str = "user_input";
const SWITCH_TO_SELECT_ID: &'static str = "switch_to_select";
const EMOTE_SELECT_ID: &'static str = "emote_select";
const SUBMIT_ID: &'static str = "submit";
const INTERACTION_CONTENT: &'static str = "Select an emote and optionally a target";

fn create_user_select<'a>(
    c: &'a mut CreateComponents,
    selected_target_value: Option<&str>,
    members: &Vec<Member>,
) -> &'a mut CreateComponents {
    c.create_action_row(|row| {
        row.create_select_menu(|menu| {
            menu.custom_id(TARGET_SELECT_ID);
            menu.placeholder("No user selected");
            menu.options(|opts| {
                for member in members {
                    opts.create_option(|o| {
                        let value = member.user.id;
                        o.label(member.display_name())
                            .value(value)
                            .default_selection(
                                selected_target_value
                                    .map(|v| v == value.to_string().as_str())
                                    .unwrap_or(false),
                            )
                    });
                }
                opts
            })
        })
    });
    c.create_action_row(|row| {
        row.create_button(|btn| {
            btn.custom_id(SWITCH_TO_INPUT_ID);
            btn.label("Switch to custom target")
        })
    })
}

fn create_user_input<'a>(
    c: &'a mut CreateComponents,
    selected_target_value: Option<&str>,
) -> &'a mut CreateComponents {
    c.create_action_row(|row| {
        row.create_input_text(|inp| {
            inp.custom_id(TARGET_INPUT_ID)
                .value(selected_target_value.unwrap_or(""))
                .style(InputTextStyle::Short)
                .label("Emote target")
        })
    });
    c.create_action_row(|row| {
        row.create_button(|btn| {
            btn.custom_id(SWITCH_TO_SELECT_ID);
            btn.label("Switch to user list")
        })
    })
}

fn create_user_select_components<'a, F>(
    create_components: &'a mut CreateComponents,
    log_message_repo: &LogMessageRepository,
    selected_emote_value: Option<&str>,
    mut create_target_component: F,
) -> &'a mut CreateComponents
where
    F: FnMut(&mut CreateComponents) -> &mut CreateComponents,
{
    create_components.create_action_row(|row| {
        row.create_select_menu(|menu| {
            menu.custom_id(EMOTE_SELECT_ID);
            menu.placeholder("No emote selected");
            menu.options(|opts| {
                // todo add more button
                for emote in log_message_repo.emote_list_by_id().take(25) {
                    opts.create_option(|o| {
                        o.label(emote).value(emote).default_selection(
                            selected_emote_value.map(|v| v == emote).unwrap_or(false),
                        )
                    });
                }
                opts
            })
        })
    });
    create_target_component(create_components);
    create_components.create_action_row(|row| {
        row.create_button(|btn| {
            btn.custom_id(SUBMIT_ID);
            btn.label("Send")
        })
    })
}

struct InteractionResult {
    emote: String,
    target: Option<Target>,
}

async fn emote_component_interaction(
    cmd: &ApplicationCommandInteraction,
    log_message_repo: &LogMessageRepository,
    context: &Context,
    members: Vec<Member>,
) -> Result<(), HandlerError> {
    trace!("creating interaction response");
    cmd.create_interaction_response(context, |res| {
        res.interaction_response_data(|data| {
            data.ephemeral(true)
                .content(INTERACTION_CONTENT)
                .components(|c| {
                    create_user_select_components(c, log_message_repo, None, |row| {
                        create_user_select(row, None, &members)
                    })
                })
        })
    })
    .await?;
    let msg = cmd.get_interaction_response(context).await?;
    trace!("awaiting interactions");
    let res = handle_interactions(context, &msg, log_message_repo, members).await?;

    let messages = log_message_repo.messages(&res.emote)?;

    // todo allow setting gender
    let origin = Character::new_from_string(
        msg.author_nick(&context)
            .await
            .unwrap_or_else(|| msg.author.name.clone()),
        Gender::Male,
        true,
        false,
    );
    trace!("message origin: {:?}", origin);
    if let Some(target_name) = &res.target {
        let target = Character::new_from_string(target_name.to_string(), Gender::Male, true, false);
        trace!("message target: {:?}", target);
        let condition_texts = extract_condition_texts(&messages.en.targeted)?;
        let answers = LogMessageAnswers::new(origin, target)?;
        send_emote(condition_texts, answers, res.target, context, &msg).await;
    } else {
        trace!("no message target");
        let condition_texts = extract_condition_texts(&messages.en.untargeted)?;
        let answers = LogMessageAnswers::new(origin, UNTARGETED_TARGET)?;

        send_emote(condition_texts, answers, None, context, &msg).await;
    };

    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum TargetComponentType {
    Input,
    Select,
}

impl TargetComponentType {
    async fn create_input_response(
        context: &Context,
        interaction: impl Deref<Target = MessageComponentInteraction>,
        log_message_repo: &LogMessageRepository,
        selected_emote_value: Option<&str>,
        selected_target_value: Option<&str>,
        _members: &Vec<Member>,
    ) -> Result<(), HandlerError> {
        interaction
            .create_interaction_response(context, |res| {
                res.kind(InteractionResponseType::UpdateMessage)
                    .interaction_response_data(|d| {
                        d.ephemeral(true)
                            .content(INTERACTION_CONTENT)
                            .components(|c| {
                                create_user_select_components(
                                    c,
                                    log_message_repo,
                                    selected_emote_value,
                                    |row| create_user_input(row, selected_target_value),
                                )
                            })
                    })
            })
            .await?;
        Ok(())
    }

    async fn create_select_response(
        context: &Context,
        interaction: impl Deref<Target = MessageComponentInteraction>,
        log_message_repo: &LogMessageRepository,
        selected_emote_value: Option<&str>,
        selected_target_value: Option<&str>,
        members: &Vec<Member>,
    ) -> Result<(), HandlerError> {
        interaction
            .create_interaction_response(context, |res| {
                res.kind(InteractionResponseType::UpdateMessage)
                    .interaction_response_data(|d| {
                        d.ephemeral(true)
                            .content(INTERACTION_CONTENT)
                            .components(|c| {
                                create_user_select_components(
                                    c,
                                    log_message_repo,
                                    selected_emote_value,
                                    |row| create_user_select(row, selected_target_value, members),
                                )
                            })
                    })
            })
            .await?;
        Ok(())
    }

    async fn create_response(
        &self,
        context: &Context,
        interaction: impl Deref<Target = MessageComponentInteraction>,
        log_message_repo: &LogMessageRepository,
        selected_emote_value: Option<&str>,
        selected_target_value: Option<&str>,
        members: &Vec<Member>,
    ) -> Result<(), HandlerError> {
        match self {
            TargetComponentType::Input => {
                Self::create_input_response(
                    context,
                    interaction,
                    log_message_repo,
                    selected_emote_value,
                    selected_target_value,
                    members,
                )
                .await
            }
            TargetComponentType::Select => {
                Self::create_select_response(
                    context,
                    interaction,
                    log_message_repo,
                    selected_emote_value,
                    selected_target_value,
                    members,
                )
                .await
            }
        }
    }
}

async fn handle_interactions(
    context: &Context,
    msg: &Message,
    log_message_repo: &LogMessageRepository,
    members: Vec<Member>,
) -> Result<InteractionResult, HandlerError> {
    let mut emote: Option<String> = None;
    let mut target: Option<Target> = None;
    let mut current_type = TargetComponentType::Select;

    while let Some(interaction) = msg
        .await_component_interactions(context)
        .collect_limit(20)
        .timeout(INTERACTION_TIMEOUT)
        .build()
        .next()
        .await
    {
        match interaction.data.custom_id.as_str() {
            SWITCH_TO_INPUT_ID => {
                debug!("switch to target input");
                target = None;
                current_type = TargetComponentType::Input;
                current_type
                    .create_response(
                        context,
                        interaction,
                        log_message_repo,
                        emote.as_deref(),
                        target.as_ref().map(|t| t.to_value()).as_deref(),
                        &members,
                    )
                    .await?;
            }
            SWITCH_TO_SELECT_ID => {
                debug!("switch to target select");
                target = None;
                current_type = TargetComponentType::Select;
                current_type
                    .create_response(
                        context,
                        interaction,
                        log_message_repo,
                        emote.as_deref(),
                        target.as_ref().map(|t| t.to_value()).as_deref(),
                        &members,
                    )
                    .await?;
            }
            EMOTE_SELECT_ID => {
                let em = interaction.data.values[0].clone();
                debug!("emote selected: {}", em);
                emote.replace(em);
                current_type
                    .create_response(
                        context,
                        interaction,
                        log_message_repo,
                        emote.as_deref(),
                        target.as_ref().map(|t| t.to_value()).as_deref(),
                        &members,
                    )
                    .await?;
            }
            TARGET_SELECT_ID => {
                let ta = interaction.data.values[0].clone();
                debug!("target selected: {}", ta);
                let user_id: UserId = match ta.parse::<u64>() {
                    Ok(id) => id,
                    Err(e) => {
                        error!("stored user id was not a number: {:?}", e);
                        return Err(HandlerError::UserNotFound);
                    }
                }
                .into();
                target.replace(Target::User(
                    members
                        .iter()
                        .map(|member| &member.user)
                        .find(|user| user.id == user_id)
                        .cloned()
                        .ok_or(HandlerError::UserNotFound)?,
                ));
                current_type
                    .create_response(
                        context,
                        interaction,
                        log_message_repo,
                        emote.as_deref(),
                        target.as_ref().map(|t| t.to_value()).as_deref(),
                        &members,
                    )
                    .await?;
            }
            TARGET_INPUT_ID => {
                let ta = interaction.data.values[0].clone();
                debug!("target input: {}", ta);
                target.replace(Target::Plain(ta));
                current_type
                    .create_response(
                        context,
                        interaction,
                        log_message_repo,
                        emote.as_deref(),
                        target.as_ref().map(|t| t.to_value()).as_deref(),
                        &members,
                    )
                    .await?;
            }
            SUBMIT_ID => {
                if let Some(em) = &emote {
                    return Ok(InteractionResult {
                        emote: em.clone(),
                        target: target.clone(),
                    });
                } else {
                    debug!("tried submitting without all necessary selections");
                }
            }
            s => {
                error!("unexpected component id: {}", s);
            }
        }
    }
    Err(HandlerError::TimeoutOrOverLimit)
}

pub async fn handle_chat_input(
    cmd: &ApplicationCommandInteraction,
    log_message_repo: &LogMessageRepository,
    context: &Context,
) -> Result<(), HandlerError> {
    trace!("finding members");
    let members = cmd
        .guild_id
        .ok_or(HandlerError::NotGuild)?
        .members(context, None, None)
        .await?;
    trace!("potential members: {:?}", members);
    emote_component_interaction(cmd, log_message_repo, context, members).await?;
    Ok(())
}

pub fn register_chat_input<'a>(
    cmd: &'a mut CreateApplicationCommand,
) -> &'a mut CreateApplicationCommand {
    cmd.name(CHAT_INPUT_COMMAND_NAME)
        .kind(CommandType::ChatInput)
        .description("Select an emote and optionally a target user")
        .dm_permission(true)
}
