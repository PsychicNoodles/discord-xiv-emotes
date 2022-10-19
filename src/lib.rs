mod commands;
mod db;

use db::Db;
use log::*;
use sqlx::PgPool;
use std::time::Duration;
use strum::IntoEnumIterator;
use thiserror::Error;

use serenity::{
    async_trait,
    constants::MESSAGE_CODE_LIMIT,
    model::{
        prelude::{command::Command, interaction::Interaction, ChannelId, Message, Ready},
        user::User,
    },
    prelude::{Context, EventHandler, GatewayIntents},
    utils::MessageBuilder,
    Client,
};
use xiv_emote_parser::{
    log_message::{
        condition::{Answers, Character, DynamicText, Gender, LogMessageAnswersError},
        parser::{extract_condition_texts, ConditionTexts, Text},
        EmoteTextError, LogMessageAnswers,
    },
    repository::{LogMessageRepository, LogMessageRepositoryError},
};

use crate::{commands::Commands, db::DbUser};

struct Handler {
    log_message_repo: LogMessageRepository,
    db: Db,
}

// untargeted messages shouldn't reference target character at all, but just in case
const UNTARGETED_TARGET: Character =
    Character::new("Godbert Manderville", Gender::Male, false, false);
const PREFIX: &str = "!";
const INTERACTION_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Error)]
pub enum HandlerError {
    #[error("Unrecognized emote ({0})")]
    UnrecognizedEmote(String),
    #[error("Unrecognized command ({0})")]
    UnrecognizedCommand(#[from] strum::ParseError),
    #[error("Command was empty")]
    EmptyCommand,
    #[error("Internal error, could not retrieve emote data")]
    EmoteData(#[from] LogMessageRepositoryError),
    #[error("Internal error, could not build response")]
    Answers(#[from] LogMessageAnswersError),
    #[error("Internal error, could not build response")]
    Extract(#[from] EmoteTextError),
    #[error("Internal error, could not build response")]
    TargetNone,
    #[error("Internal error, could not build response")]
    Db(#[from] db::DbError),
    #[error("Failed to send message")]
    Send(#[from] serenity::Error),
    #[error("Expected to be in a guild channel")]
    NotGuild,
    #[error("Timed out or had too many inputs")]
    TimeoutOrOverLimit,
    #[error("Couldn't find user")]
    UserNotFound,
    #[error("Unexpected data received from server")]
    UnexpectedData,
}

pub fn split_by_max_message_len(
    prefix: impl AsRef<str>,
    mut body: impl Iterator<Item = String>,
) -> Vec<String> {
    let mut res = vec![];
    let mut msg = if let Some(item) = body.next() {
        item
    } else {
        return res;
    };
    while let Some(item) = body.next() {
        msg.push_str(", ");

        // todo count with codepoints rather than String len?
        if prefix.as_ref().len() + " (xx/xx): ".len() + msg.len() + item.len() + ", ".len()
            > MESSAGE_CODE_LIMIT
        {
            res.push(msg);
            msg = String::new();
        }

        msg.push_str(&item);
    }
    res.push(msg);
    let count = res.len();
    res.iter_mut().enumerate().for_each(|(i, m)| {
        m.insert_str(0, &format!("{} ({}/{}): ", prefix.as_ref(), i + 1, count));
    });
    trace!("res: {:?}", res);
    res
}

async fn check_other_cmd(
    mparts: &[&str],
    log_message_repo: &LogMessageRepository,
    context: &Context,
    msg: &Message,
) -> Result<bool, HandlerError> {
    match mparts[..] {
        [_cmd] if _cmd == "emotes" => {
            trace!("emotes command");
            let emote_list: Vec<_> = log_message_repo.emote_list_by_id().cloned().collect();

            const EMOTE_LIST_PREFIX: &str = "List of emotes";
            let results = split_by_max_message_len(EMOTE_LIST_PREFIX, emote_list.into_iter());
            debug!("emotes response is {} messages long", results.len());

            for res in results {
                msg.reply(context, res).await?;
            }
            Ok(true)
        }
        [_cmd] if _cmd == "help" => {
            trace!("help command");
            msg.reply(context, format!("Use emotes from FFXIV in chat, optionally with a (mentionable) target! Use {}emotes for a list of options.", PREFIX)).await?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

async fn process_input(
    mparts: &[&str],
    log_message_repo: &LogMessageRepository,
    db: &Db,
    context: &Context,
    msg: &Message,
) -> Result<(), HandlerError> {
    if check_other_cmd(mparts, log_message_repo, context, msg).await? {
        debug!("non-emote command");
        return Ok(());
    }

    let (emote, mention) = mparts.split_first().ok_or(HandlerError::EmptyCommand)?;
    let emote = ["/", emote].concat();
    let mention = if mention.is_empty() {
        None
    } else {
        Some(mention.join(" "))
    };

    trace!("parsed command and mention: {:?} {:?}", emote, mention);

    let user = db.find_user(msg.author.id.to_string()).await?;
    let language = user
        .as_ref()
        .map(DbUser::language)
        .cloned()
        .unwrap_or_default();
    let gender = user
        .as_ref()
        .map(DbUser::gender)
        .cloned()
        .unwrap_or_default();
    trace!("language is {:?}, gender is {:?}", language, gender);

    match (&emote, mention) {
        (emote, Some(mention)) if log_message_repo.contains_emote(emote) => {
            debug!("emote with mention");
            let messages = log_message_repo.messages(emote)?;
            let origin = Character::new_from_string(
                msg.author_nick(&context)
                    .await
                    .unwrap_or_else(|| msg.author.name.clone()),
                gender.into(),
                true,
                false,
            );
            trace!("message origin: {:?}", origin);
            let target = Character::new_from_string(mention.to_string(), Gender::Male, true, false);
            trace!("message target: {:?}", target);
            let answers = LogMessageAnswers::new(origin, target)?;
            let condition_texts =
                extract_condition_texts(&language.with_emote_data(&messages).targeted)?;
            send_emote(
                condition_texts,
                answers,
                Some(mention),
                context,
                SendTargetType::Message(msg),
            )
            .await?;
            Ok(())
        }
        (emote, None) if log_message_repo.contains_emote(emote) => {
            debug!("emote without mention");
            let messages = log_message_repo.messages(emote)?;
            let origin = Character::new_from_string(
                msg.author_nick(&context)
                    .await
                    .unwrap_or_else(|| msg.author.name.clone()),
                gender.into(),
                true,
                false,
            );
            trace!("message origin: {:?}", origin);
            let answers = LogMessageAnswers::new(origin, UNTARGETED_TARGET)?;
            let condition_texts =
                extract_condition_texts(&language.with_emote_data(&messages).untargeted)?;
            send_emote(
                condition_texts,
                answers,
                None,
                context,
                SendTargetType::Message(msg),
            )
            .await?;
            Ok(())
        }
        (emote, _) => Err(HandlerError::UnrecognizedEmote(emote.to_string())),
    }
}

// async fn determine_mention(msg: &Message, context: &Context) -> Option<Target> {
//     if let Some(user) = msg.mentions.first() {
//         trace!("mention appears to be a user");
//         Some(Target::User(user.clone()))
//     } else if let Some(role_id) = msg.mention_roles.first() {
//         trace!("mention appears to be a role");
//         msg.guild(context.cache.clone())?
//             .roles
//             .get(role_id)
//             .cloned()
//             .map(Target::Role)
//     } else if msg.mention_everyone {
//         trace!("mention appears to be everyone");
//         Some(Target::Plain("everyone in the vicinity".to_string()))
//     } else {
//         trace!("no mention found");
//         None
//     }
// }

#[derive(Debug, Clone, Copy)]
enum SendTargetType<'a> {
    Message(&'a Message),
    Channel {
        channel: &'a ChannelId,
        author: &'a User,
    },
}

async fn send_emote<'a>(
    condition_texts: ConditionTexts,
    answers: impl Answers,
    target_name: Option<String>,
    context: &Context,
    target_type: SendTargetType<'a>,
) -> Result<(), HandlerError> {
    let mut msg_builder = MessageBuilder::new();
    let author = match target_type {
        SendTargetType::Message(m) => &m.author,
        SendTargetType::Channel { author, .. } => author,
    };
    let mut errs: Vec<_> = condition_texts
        .map_texts_mut(&answers, |text| {
            match text {
                Text::Dynamic(d) => match d {
                    DynamicText::NpcOriginName
                    | DynamicText::PlayerOriginNameEn
                    | DynamicText::PlayerOriginNameJp => msg_builder.mention(author),
                    DynamicText::NpcTargetName
                    | DynamicText::PlayerTargetNameEn
                    | DynamicText::PlayerTargetNameJp => match &target_name {
                        Some(n) => msg_builder.push(n),
                        None => return Some(HandlerError::TargetNone),
                    },
                },
                Text::Static(s) => msg_builder.push(s),
            };
            None
        })
        .collect();
    if !errs.is_empty() {
        error!("errors during text processing: {:?}", errs);
        return Err(errs.remove(0));
    }
    if let Err(e) = match target_type {
        SendTargetType::Message(msg) => msg.reply(context, msg_builder.build()).await,
        SendTargetType::Channel { channel, .. } => {
            channel
                .send_message(context, |c| c.content(msg_builder.build()))
                .await
        }
    } {
        error!("failed to send emote message: {:?}", e);
    }
    Ok(())
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, context: Context, msg: Message) {
        trace!("incoming message: {:?}", msg);
        if !msg.is_own(&context) && msg.content.starts_with(PREFIX) {
            let mut mparts: Vec<_> = msg.content.split_whitespace().collect();
            if let Some(first) = mparts.get_mut(0) {
                *first = first.get(1..).unwrap_or(first);
            }
            debug!("message parts: {:?}", mparts);
            match process_input(&mparts, &self.log_message_repo, &self.db, &context, &msg).await {
                Ok(v) => v,
                Err(err) => {
                    error!("error during message processing: {:?}", err);
                    if let Err(e) = msg.reply(context, err.to_string()).await {
                        error!(
                            "could not send follow-up message ({}): {:?}",
                            err.to_string(),
                            e
                        );
                    }
                    return;
                }
            }
        }
    }

    async fn interaction_create(&self, context: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(cmd) = interaction {
            trace!("incoming application command: {:?}", cmd);

            if let Err(err) = match Commands::try_from(cmd.data.name.as_str()) {
                Ok(Commands::EmoteSelect) => {
                    commands::emote_select::handle_chat_input(
                        &cmd,
                        &self.log_message_repo,
                        &context,
                    )
                    .await
                }
                Ok(Commands::UserSettings) => {
                    commands::user_settings::handle_chat_input(&cmd, &self.db, &context).await
                }
                Err(e) => Err(HandlerError::UnrecognizedCommand(e)),
            } {
                error!("error during interaction processing: {:?}", err);
                if let Err(e) = cmd
                    .create_followup_message(&context, |msg| msg.content(err.to_string()))
                    .await
                {
                    error!(
                        "could not send follow-up message ({}): {:?}",
                        err.to_string(),
                        e
                    );
                }
                return;
            }
        }
    }

    async fn ready(&self, context: Context, ready: Ready) {
        info!("{} is connected", ready.user.name);

        if let Err(err) = Command::set_global_application_commands(&context, |create| {
            Commands::iter().for_each(|cmd| {
                create.create_application_command(cmd.register());
            });
            create
        })
        .await
        {
            error!("error during setup: {:?}", err);
        }
    }
}

pub async fn setup_client(token: String, pool: PgPool) -> Client {
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_MEMBERS;
    let log_message_repo = LogMessageRepository::from_xivapi(None)
        .await
        .expect("couldn't load log message data from xivapi");
    info!(
        "repo initialized with emotes: {:?}",
        log_message_repo.emote_list_by_id().collect::<Vec<_>>()
    );
    Client::builder(&token, intents)
        .event_handler(Handler {
            log_message_repo,
            db: Db(pool),
        })
        .await
        .expect("error creating client")
}

// #[shuttle_service::main]
// async fn shuttle_main(
//     #[shuttle_secrets::Secrets] secret_store: SecretStore,
//     #[shuttle_shared_db::Postgres] pool: PgPool,
// ) -> shuttle_service::ShuttleSerenity {
//     let token = secret_store
//         .get("DISCORD_TOKEN")
//         .expect("could not find discord token");

//     sqlx::migrate!()
//         .run(&pool)
//         .await
//         .expect("could not migrate db");

//     let client = setup_client(token).await;
//     Ok(client)
// }
