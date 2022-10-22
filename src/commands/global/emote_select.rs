use async_trait::async_trait;
use futures::stream::StreamExt;
use log::*;
use serenity::{
    builder::{CreateApplicationCommand, CreateComponents, CreateInteractionResponse},
    model::{
        guild::Member,
        id::UserId,
        prelude::{
            command::CommandType,
            component::{ActionRowComponent, InputTextStyle},
            interaction::{
                application_command::ApplicationCommandInteraction, InteractionResponseType,
            },
            Message,
        },
        user::User,
    },
    prelude::{Context, Mentionable},
};
use thiserror::Error;

use crate::{commands::AppCmd, HandlerError, INTERACTION_TIMEOUT, UNTARGETED_TARGET};

use super::GlobalCommands;

const INPUT_TARGET_MODAL: &str = "input_target_modal";
const INPUT_TARGET_COMPONENT: &str = "input_target_input";

enum Ids {
    TargetSelect,
    InputTargetBtn,
    EmoteSelect,
    EmotePrevBtn,
    EmoteNextBtn,
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
            Ids::TargetSelect => "user_select",
            Ids::InputTargetBtn => "input_target_btn",
            Ids::EmoteSelect => "emote_select",
            Ids::EmotePrevBtn => "prev_emotes",
            Ids::EmoteNextBtn => "next_emotes",
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
            "user_select" => Ok(Ids::TargetSelect),
            "input_target_btn" => Ok(Ids::InputTargetBtn),
            "emote_select" => Ok(Ids::EmoteSelect),
            "prev_emotes" => Ok(Ids::EmotePrevBtn),
            "next_emotes" => Ok(Ids::EmoteNextBtn),
            "submit" => Ok(Ids::Submit),
            s => Err(InvalidComponentId(s.to_string())),
        }
    }
}

#[derive(Debug, Clone)]
enum Target {
    User(UserId),
    // Role(Role),
    Plain(String),
}

impl Default for Target {
    fn default() -> Self {
        Target::Plain(UNTARGETED_TARGET.name.into_owned())
    }
}

impl ToString for Target {
    fn to_string(&self) -> String {
        match self {
            Target::User(u) => u.mention().to_string(),
            // Target::Role(r) => r.name.clone(),
            Target::Plain(s) => s.to_string(),
        }
    }
}

const INTERACTION_CONTENT: &str = "Select an emote and optionally a target";

// max number of select menu options
const EMOTE_LIST_OFFSET_STEP: usize = 25;

#[derive(Debug, Clone)]
struct UserInfo {
    name: String,
    id: UserId,
}

impl From<Member> for UserInfo {
    fn from(m: Member) -> Self {
        UserInfo {
            name: m.display_name().into_owned(),
            id: m.user.id,
        }
    }
}

impl From<User> for UserInfo {
    fn from(u: User) -> Self {
        UserInfo {
            name: u.name,
            id: u.id,
        }
    }
}

impl From<&User> for UserInfo {
    fn from(u: &User) -> Self {
        UserInfo {
            name: u.name.clone(),
            id: u.id,
        }
    }
}

fn create_user_select<'a>(
    c: &'a mut CreateComponents,
    selected_target_value: Option<&Target>,
    members: &Vec<UserInfo>,
) -> &'a mut CreateComponents {
    c.create_action_row(|row| {
        row.create_select_menu(|menu| {
            menu.custom_id(Ids::TargetSelect)
                .placeholder(
                    selected_target_value
                        .and_then(|t| match t {
                            Target::Plain(s) => Some(s.as_str()),
                            _ => None,
                        })
                        .unwrap_or("No user selected"),
                )
                .options(|opts| {
                    for member in members {
                        opts.create_option(|o| {
                            let value = member.id;
                            o.label(&member.name).value(value).default_selection(
                                selected_target_value
                                    .map(|t| matches!(t, Target::User(u) if *u == value))
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
            btn.custom_id(Ids::InputTargetBtn);
            btn.label("Input custom target")
        })
    })
}

fn create_user_select_components<'a, F>(
    create_components: &'a mut CreateComponents,
    emote_list: &Vec<&String>,
    emote_list_offset: Option<usize>,
    selected_emote_value: Option<&str>,
    mut create_target_component: F,
) -> &'a mut CreateComponents
where
    F: FnMut(&mut CreateComponents) -> &mut CreateComponents,
{
    create_components.create_action_row(|row| {
        row.create_select_menu(|menu| {
            menu.custom_id(Ids::EmoteSelect)
                .placeholder("No emote selected")
                .options(|opts| {
                    for emote in emote_list
                        .iter()
                        .skip(emote_list_offset.unwrap_or(0))
                        .take(EMOTE_LIST_OFFSET_STEP)
                    {
                        opts.create_option(|o| {
                            o.label(emote).value(emote).default_selection(
                                selected_emote_value
                                    .map(|v| v == emote.as_str())
                                    .unwrap_or(false),
                            )
                        });
                    }
                    opts
                })
        })
    });
    create_components.create_action_row(|row| {
        row.create_button(|btn| {
            btn.custom_id(Ids::EmotePrevBtn)
                .label("Previous emote page")
                .disabled(
                    emote_list_offset
                        .map(|off| off < EMOTE_LIST_OFFSET_STEP)
                        .unwrap_or(true),
                )
        });
        row.create_button(|btn| {
            btn.custom_id(Ids::EmoteNextBtn)
                .label("Next emote page")
                .disabled(
                    emote_list_offset
                        .map(|off| off + EMOTE_LIST_OFFSET_STEP >= emote_list.len())
                        .unwrap_or(false),
                )
        })
    });
    create_target_component(create_components);
    create_components.create_action_row(|row| {
        row.create_button(|btn| {
            btn.custom_id(Ids::Submit);
            btn.label("Send")
        })
    })
}

struct InteractionResult {
    emote: String,
    target: Option<Target>,
}

fn interaction_response_content(emote_list_len: usize, emote_list_offset: Option<usize>) -> String {
    format!(
        "{} (page {} of {})",
        INTERACTION_CONTENT,
        emote_list_offset
            .map(|off| off / EMOTE_LIST_OFFSET_STEP)
            .unwrap_or(0)
            + 1,
        emote_list_len / EMOTE_LIST_OFFSET_STEP + 1
    )
}

fn create_response<'a, 'b>(
    res: &'a mut CreateInteractionResponse<'b>,
    kind: InteractionResponseType,
    emote_list: &Vec<&String>,
    emote_list_offset: Option<usize>,
    selected_emote_value: Option<&str>,
    selected_target_value: Option<&Target>,
    members: &Vec<UserInfo>,
) -> &'a mut CreateInteractionResponse<'b> {
    res.kind(kind).interaction_response_data(|d| {
        d.ephemeral(true)
            .content(interaction_response_content(
                emote_list.len(),
                emote_list_offset,
            ))
            .components(|c| {
                create_user_select_components(
                    c,
                    emote_list,
                    emote_list_offset,
                    selected_emote_value,
                    |row| create_user_select(row, selected_target_value, members),
                )
            })
    })
}

async fn handle_interactions(
    context: &Context,
    msg: &Message,
    emote_list: &Vec<&String>,
    members: Vec<UserInfo>,
) -> Result<InteractionResult, HandlerError> {
    let mut emote: Option<String> = None;
    let mut emote_list_offset: Option<usize> = None;
    let mut target: Option<Target> = None;

    while let Some(interaction) = msg
        .await_component_interactions(context)
        .collect_limit(20)
        .timeout(INTERACTION_TIMEOUT)
        .build()
        .next()
        .await
    {
        trace!("incoming interaction: {:?}", interaction);
        match Ids::try_from(interaction.data.custom_id.as_str()) {
            Ok(Ids::InputTargetBtn) => {
                debug!("target input");
                interaction
                    .create_interaction_response(context, |res| {
                        res.kind(InteractionResponseType::Modal)
                            .interaction_response_data(|d| {
                                d.content("Input target name")
                                    .components(|c| {
                                        c.create_action_row(|row| {
                                            row.create_input_text(|inp| {
                                                inp.custom_id(INPUT_TARGET_COMPONENT)
                                                    .style(InputTextStyle::Short)
                                                    .label("Target name")
                                            })
                                        })
                                    })
                                    .title("Custom target input")
                                    .custom_id(INPUT_TARGET_MODAL)
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
                            trace!("setting target to: {}", cmp.value);
                            target = Some(Target::Plain(cmp.value.clone()));
                            modal_interaction
                                .create_interaction_response(context, |res| {
                                    create_response(
                                        res,
                                        InteractionResponseType::UpdateMessage,
                                        emote_list,
                                        emote_list_offset,
                                        emote.as_deref(),
                                        target.as_ref(),
                                        &members,
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
            Ok(Ids::EmoteSelect) => {
                let em = interaction.data.values[0].clone();
                debug!("emote selected: {}", em);
                emote.replace(em);
            }
            Ok(Ids::EmotePrevBtn) => {
                debug!("previous emote list page");
                emote_list_offset = match emote_list_offset {
                    None => None,
                    Some(_o) if _o <= EMOTE_LIST_OFFSET_STEP => None,
                    Some(o) => Some(o - EMOTE_LIST_OFFSET_STEP),
                };
            }
            Ok(Ids::EmoteNextBtn) => {
                debug!("next emote list page");
                emote_list_offset = match emote_list_offset {
                    None => Some(EMOTE_LIST_OFFSET_STEP),
                    Some(_o) if _o + EMOTE_LIST_OFFSET_STEP >= emote_list.len() => Some(_o),
                    Some(o) => Some(o + EMOTE_LIST_OFFSET_STEP),
                };
            }
            Ok(Ids::TargetSelect) => {
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
                        .map(|member| member.id)
                        .find(|user| *user == user_id)
                        .ok_or(HandlerError::UserNotFound)?,
                ));
            }
            Ok(Ids::Submit) => {
                if let Some(em) = &emote {
                    interaction
                        .create_interaction_response(context, |res| {
                            res.kind(InteractionResponseType::UpdateMessage)
                                .interaction_response_data(|d| {
                                    d.content(format!(
                                        "Emote sent! ({}{})",
                                        em,
                                        if let Some(t) = &target {
                                            [" ".to_string(), t.to_string()].concat()
                                        } else {
                                            "".to_string()
                                        }
                                    ))
                                    .components(|cmp| cmp)
                                })
                        })
                        .await?;
                    return Ok(InteractionResult {
                        emote: em.clone(),
                        target: target.clone(),
                    });
                } else {
                    debug!("tried submitting without all necessary selections");
                }
            }
            Err(e) => {
                error!("unexpected component id: {}", e);
            }
        }

        interaction
            .create_interaction_response(context, |res| {
                create_response(
                    res,
                    InteractionResponseType::UpdateMessage,
                    emote_list,
                    emote_list_offset,
                    emote.as_deref(),
                    target.as_ref(),
                    &members,
                )
            })
            .await?;
    }
    Err(HandlerError::TimeoutOrOverLimit)
}

pub struct EmoteSelectCmd;

#[async_trait]
impl AppCmd for EmoteSelectCmd {
    fn to_application_command() -> CreateApplicationCommand
    where
        Self: Sized,
    {
        let mut cmd = CreateApplicationCommand::default();
        cmd.name(GlobalCommands::EmoteSelect)
            .kind(CommandType::ChatInput)
            .description("Select an emote and optionally a target user")
            .dm_permission(true);
        cmd
    }

    async fn handle(
        cmd: &ApplicationCommandInteraction,
        handler: &crate::Handler,
        context: &Context,
    ) -> Result<(), HandlerError>
    where
        Self: Sized,
    {
        trace!("finding members");

        let members = if let Some(guild_id) = cmd.guild_id {
            guild_id
                .members(context, None, None)
                .await?
                .into_iter()
                .map(UserInfo::from)
                .collect()
        } else {
            vec![
                UserInfo::from(&cmd.user),
                UserInfo::from(User::from(context.cache.current_user())),
            ]
        };
        trace!("potential members: {:?}", members);

        trace!("creating interaction response");
        let emote_list: Vec<_> = handler.log_message_repo.emote_list_by_id().collect();
        cmd.create_interaction_response(context, |res| {
            create_response(
                res,
                InteractionResponseType::ChannelMessageWithSource,
                &emote_list,
                None,
                None,
                None,
                &members,
            )
        })
        .await?;
        let msg = cmd.get_interaction_response(context).await?;

        trace!("awaiting interactions");
        let res = handle_interactions(context, &msg, &emote_list, members).await?;

        let user = handler.db.find_user(cmd.user.id).await?;
        let guild = if let Some(guild_id) = cmd.guild_id {
            handler.db.find_guild(guild_id).await?
        } else {
            None
        };

        let body = handler.build_emote_message(
            &res.emote,
            user,
            &msg.author,
            res.target.map(|t| t.to_string()).as_deref(),
            guild,
        )?;
        cmd.channel_id
            .send_message(context, |m| m.content(body))
            .await?;

        Ok(())
    }
}
