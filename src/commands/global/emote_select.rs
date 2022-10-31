use std::sync::Arc;

use async_trait::async_trait;
use futures::stream::StreamExt;
use serenity::{
    builder::{CreateApplicationCommand, CreateInteractionResponse},
    model::{
        guild::Member,
        id::UserId,
        prelude::{
            command::CommandType,
            component::{ActionRowComponent, InputTextStyle},
            interaction::{
                application_command::ApplicationCommandInteraction,
                message_component::MessageComponentInteraction, InteractionResponseType,
            },
            Message,
        },
        user::User,
    },
    prelude::{Context, Mentionable},
};
use thiserror::Error;
use tracing::*;

use crate::{
    commands::AppCmd,
    db::models::DbUser,
    util::{CreateApplicationCommandExt, LocalizedString},
    HandlerError, MessageDbData, INTERACTION_TIMEOUT, UNTARGETED_TARGET,
};

pub const CONTENT: LocalizedString = LocalizedString {
    en: "Select an emote and optionally a target",
    ja: "エモートを選択してターゲットを任意選択して送信",
};
pub const NO_USER_SELECTED: LocalizedString = LocalizedString {
    en: "No user selected",
    ja: "ユーザー未選択",
};
pub const INPUT_USER_BTN: LocalizedString = LocalizedString {
    en: "Input custom target",
    ja: "ターゲット指定入力",
};
pub const INPUT_TARGET_MODAL_CONTENT: LocalizedString = LocalizedString {
    en: "Input target name",
    ja: "ターゲットの名前を入力してください",
};
pub const INPUT_TARGET_MODAL_INPUT: LocalizedString = LocalizedString {
    en: "Target name",
    ja: "ターゲットの名前",
};
pub const INPUT_TARGET_MODAL_TITLE: LocalizedString = LocalizedString {
    en: "Custom emote target",
    ja: "エモートのターゲット指定",
};
pub const NO_EMOTE_SELECTED: LocalizedString = LocalizedString {
    en: "No emote selected",
    ja: "エモート未選択",
};
pub const PREV_EMOTE_PAGE: LocalizedString = LocalizedString {
    en: "Previous emote page",
    ja: "前のエモートページへ",
};
pub const NEXT_EMOTE_PAGE: LocalizedString = LocalizedString {
    en: "Next emote page",
    ja: "次のエモートページへ",
};
pub const SEND_BTN: LocalizedString = LocalizedString {
    en: "Send",
    ja: "送信",
};
pub const EMOTE_SENT: LocalizedString = LocalizedString {
    en: "Emote sent!",
    ja: "送信しました！",
};
pub const NAME: LocalizedString = LocalizedString {
    en: "emote-select",
    ja: "エモート選択",
};
pub const DESC: LocalizedString = LocalizedString {
    en: "Interactively select and send an emote with an optional target user",
    // todo figure out better translation for this
    ja: "エモートを選択してターゲットを任意選択して送信",
};

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

impl Target {
    fn user_id(&self) -> Option<&UserId> {
        match self {
            Target::User(u) => Some(u),
            Target::Plain(_) => None,
        }
    }
}

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

struct InteractionResult {
    emote: String,
    target: Option<Target>,
}

fn interaction_response_content(
    emote_list_len: usize,
    emote_list_offset: Option<usize>,
    user: &DbUser,
) -> String {
    format!(
        "{} ({}/{})",
        CONTENT.for_user(user),
        emote_list_offset
            .map(|off| off / EMOTE_LIST_OFFSET_STEP)
            .unwrap_or(0)
            + 1,
        emote_list_len / EMOTE_LIST_OFFSET_STEP + 1
    )
}

#[derive(Debug, Clone, Default)]
struct Selection {
    emote_list_offset: Option<usize>,
    selected_emote_value: Option<String>,
    selected_target_value: Option<Target>,
}

#[instrument(skip(res))]
fn create_response<'a, 'b>(
    res: &'a mut CreateInteractionResponse<'b>,
    kind: InteractionResponseType,
    user: &DbUser,
    emote_list: &[impl AsRef<str> + std::fmt::Debug],
    selection: &Selection,
    members: &[UserInfo],
) -> &'a mut CreateInteractionResponse<'b> {
    res.kind(kind).interaction_response_data(|d| {
        d.ephemeral(true)
            .content(interaction_response_content(
                emote_list.len(),
                selection.emote_list_offset,
                user,
            ))
            .components(|c| {
                c.create_action_row(|row| {
                    row.create_select_menu(|menu| {
                        menu.custom_id(Ids::EmoteSelect)
                            .placeholder(NO_EMOTE_SELECTED.for_user(user))
                            .options(|opts| {
                                for emote in emote_list
                                    .iter()
                                    .skip(selection.emote_list_offset.unwrap_or(0))
                                    .take(EMOTE_LIST_OFFSET_STEP)
                                {
                                    let emote = emote.as_ref();
                                    opts.create_option(|o| {
                                        o.label(emote).value(emote).default_selection(
                                            selection
                                                .selected_emote_value
                                                .as_ref()
                                                .map(|v| v == emote)
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
                        btn.custom_id(Ids::EmotePrevBtn)
                            .label(PREV_EMOTE_PAGE.for_user(user))
                            .disabled(
                                selection
                                    .emote_list_offset
                                    .map(|off| off < EMOTE_LIST_OFFSET_STEP)
                                    .unwrap_or(true),
                            )
                    });
                    row.create_button(|btn| {
                        btn.custom_id(Ids::EmoteNextBtn)
                            .label(NEXT_EMOTE_PAGE.for_user(user))
                            .disabled(
                                selection
                                    .emote_list_offset
                                    .map(|off| off + EMOTE_LIST_OFFSET_STEP >= emote_list.len())
                                    .unwrap_or(false),
                            )
                    })
                });
                c.create_action_row(|row| {
                    row.create_select_menu(|menu| {
                        menu.custom_id(Ids::TargetSelect)
                            .placeholder(
                                selection
                                    .selected_target_value
                                    .as_ref()
                                    .and_then(|t| match t {
                                        Target::Plain(s) => Some(s.as_str()),
                                        _ => None,
                                    })
                                    .unwrap_or_else(|| NO_USER_SELECTED.for_user(user)),
                            )
                            .options(|opts| {
                                for member in members {
                                    opts.create_option(|o| {
                                        let value = member.id;
                                        o.label(&member.name).value(value).default_selection(
                                            selection
                                                .selected_target_value
                                                .as_ref()
                                                .map(
                                                    |t| matches!(t, Target::User(u) if *u == value),
                                                )
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
                        btn.custom_id(Ids::InputTargetBtn)
                            .label(INPUT_USER_BTN.for_user(user))
                    })
                });
                c.create_action_row(|row| {
                    row.create_button(|btn| {
                        btn.custom_id(Ids::Submit).label(SEND_BTN.for_user(user))
                    })
                })
            })
    })
}

#[instrument(skip(context))]
async fn handle_interaction(
    context: &Context,
    msg: &Message,
    user: &DbUser,
    emote_list: &[impl AsRef<str> + std::fmt::Debug],
    members: &[UserInfo],
    interaction: Arc<MessageComponentInteraction>,
    selection: &mut Selection,
) -> Result<Option<InteractionResult>, HandlerError> {
    match Ids::try_from(interaction.data.custom_id.as_str()) {
        Ok(Ids::InputTargetBtn) => {
            debug!("target input");
            let span = debug_span!("target_input_modal_interaction");
            async move {
                interaction
                    .create_interaction_response(context, |res| {
                        res.kind(InteractionResponseType::Modal)
                            .interaction_response_data(|d| {
                                d.content(INPUT_TARGET_MODAL_CONTENT.for_user(user))
                                    .components(|c| {
                                        c.create_action_row(|row| {
                                            row.create_input_text(|inp| {
                                                inp.custom_id(INPUT_TARGET_COMPONENT)
                                                    .style(InputTextStyle::Short)
                                                    .label(INPUT_TARGET_MODAL_INPUT.for_user(user))
                                            })
                                        })
                                    })
                                    .title(INPUT_TARGET_MODAL_TITLE.for_user(user))
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
                            trace!(target = cmp.value, "setting target");
                            selection.selected_target_value =
                                Some(Target::Plain(cmp.value.clone()));
                            modal_interaction
                                .create_interaction_response(context, |res| {
                                    create_response(
                                        res,
                                        InteractionResponseType::UpdateMessage,
                                        user,
                                        emote_list,
                                        selection,
                                        members,
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
        Ok(Ids::EmoteSelect) => {
            let em = interaction.data.values[0].clone();
            debug!(em, "emote selected");
            selection.selected_emote_value.replace(em);
        }
        Ok(Ids::EmotePrevBtn) => {
            debug!(selection.emote_list_offset, "previous emote list page");
            selection.emote_list_offset = match selection.emote_list_offset {
                None => None,
                Some(_o) if _o <= EMOTE_LIST_OFFSET_STEP => None,
                Some(o) => Some(o - EMOTE_LIST_OFFSET_STEP),
            };
        }
        Ok(Ids::EmoteNextBtn) => {
            debug!(selection.emote_list_offset, "next emote list page");
            selection.emote_list_offset = match selection.emote_list_offset {
                None => Some(EMOTE_LIST_OFFSET_STEP),
                Some(_o) if _o + EMOTE_LIST_OFFSET_STEP >= emote_list.len() => Some(_o),
                Some(o) => Some(o + EMOTE_LIST_OFFSET_STEP),
            };
        }
        Ok(Ids::TargetSelect) => {
            let ta = interaction.data.values[0].clone();
            debug!(ta, "target selected");
            let user_id: UserId = match ta.parse::<u64>() {
                Ok(id) => id,
                Err(err) => {
                    error!(?err, "stored user id was not a number");
                    return Err(HandlerError::UserNotFound);
                }
            }
            .into();
            selection.selected_target_value.replace(Target::User(
                members
                    .iter()
                    .map(|member| member.id)
                    .find(|user| *user == user_id)
                    .ok_or(HandlerError::UserNotFound)?,
            ));
        }
        Ok(Ids::Submit) => {
            if let Some(emote) = selection.selected_emote_value.take() {
                return Ok(Some(InteractionResult {
                    emote,
                    target: selection.selected_target_value.take(),
                }));
            } else {
                debug!("tried submitting without all necessary selections");
            }
        }
        Err(err) => {
            error!(?err, "unexpected component id");
        }
    }

    interaction
        .create_interaction_response(context, |res| {
            create_response(
                res,
                InteractionResponseType::UpdateMessage,
                user,
                emote_list,
                selection,
                members,
            )
        })
        .await?;

    Ok(None)
}

async fn handle_interactions(
    context: &Context,
    msg: &Message,
    user: &DbUser,
    emote_list: &[impl AsRef<str> + std::fmt::Debug],
    members: Vec<UserInfo>,
) -> Result<InteractionResult, HandlerError> {
    let mut selection = Selection::default();

    while let Some(interaction) = msg
        .await_component_interactions(context)
        .collect_limit(20)
        .timeout(INTERACTION_TIMEOUT)
        .build()
        .next()
        .await
    {
        if let Some(res) = handle_interaction(
            context,
            msg,
            user,
            emote_list,
            &members,
            interaction,
            &mut selection,
        )
        .await?
        {
            return Ok(res);
        }
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

        let user_settings = message_db_data.determine_user_settings().await?;

        info!(?members, "emote select command");

        let emote_list: Vec<_> = handler.log_message_repo.emote_list_by_id().collect();
        cmd.create_interaction_response(context, |res| {
            create_response(
                res,
                InteractionResponseType::ChannelMessageWithSource,
                &user_settings,
                emote_list.as_slice(),
                &Selection::default(),
                &members,
            )
        })
        .await?;
        let msg = cmd.get_interaction_response(context).await?;

        trace!("awaiting interactions");
        let res = handle_interactions(
            context,
            &msg,
            &user_settings,
            emote_list.as_slice(),
            members,
        )
        .await?;

        let messages = handler.log_message_repo.messages(&res.emote)?;
        let body = handler
            .build_emote_message(
                messages,
                message_db_data,
                &cmd.user,
                res.target.as_ref().map(|t| t.to_string()).as_deref(),
            )
            .await?;
        debug!(body, "processed selected emote");
        cmd.channel_id
            .send_message(context, |m| m.content(body))
            .await?;
        handler
            .log_emote(
                &cmd.user.id,
                cmd.guild_id.as_ref(),
                res.target
                    .as_ref()
                    .map(Target::user_id)
                    .flatten()
                    .map(|id| *id)
                    .iter(),
                messages,
            )
            .await?;

        cmd.edit_original_interaction_response(context, |d| {
            d.content(format!(
                "{} ({}{})",
                EMOTE_SENT.for_user(&user_settings),
                res.emote,
                if let Some(t) = &res.target {
                    [" ".to_string(), t.to_string()].concat()
                } else {
                    "".to_string()
                }
            ))
            .components(|cmp| cmp)
        })
        .await?;

        Ok(())
    }

    fn name() -> LocalizedString {
        NAME
    }
}
