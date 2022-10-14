use dotenv::dotenv;
use log::*;
use std::env;
use thiserror::Error;

use serenity::{
    async_trait,
    model::prelude::Message,
    prelude::{Context, EventHandler, GatewayIntents},
    utils::MessageBuilder,
    Client,
};
use xiv_emote_parser::{
    log_message::{
        condition::{Character, DynamicText, Gender, LogMessageAnswersError},
        parser::{extract_condition_texts, ConditionTexts, Text},
        EmoteTextError, LogMessageAnswers,
    },
    repository::{LogMessageRepository, LogMessageRepositoryError},
};

struct Handler {
    log_message_repo: LogMessageRepository,
}

// untargeted messages shouldn't reference target character at all, but just in case
const UNTARGETED_TARGET: Character =
    Character::new("Godbert Manderville", Gender::Male, false, false);
const PREFIX: &'static str = "!";

#[derive(Debug, Error)]
enum HandlerError {
    #[error("Unrecognized emote ({0})")]
    UnrecognizedEmote(String),
    #[error("Invalid format ({0})")]
    InvalidFormat(String),
    #[error("Internal error, could not retrieve emote data")]
    EmoteData(#[from] LogMessageRepositoryError),
    #[error("Internal error, could not build response")]
    Answers(#[from] LogMessageAnswersError),
    #[error("Internal error, could not build response")]
    Extract(#[from] EmoteTextError),
    #[error("Failed to send message")]
    Send(#[from] serenity::Error),
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
            msg.reply(
                context,
                format!("List of emotes: {}", emote_list.join(", ")),
            )
            .await?;
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
    context: &Context,
    msg: &Message,
) -> Result<Option<(ConditionTexts, LogMessageAnswers, String)>, HandlerError> {
    if check_other_cmd(mparts, log_message_repo, context, msg).await? {
        debug!("non-emote command");
        return Ok(None);
    }

    let (cmd, mention) = match mparts[..] {
        [e, m] => Ok((["/", e].concat(), Some(m))),
        [e] => Ok((["/", e].concat(), None)),
        _ => Err(HandlerError::InvalidFormat(mparts.join(" "))),
    }?;
    trace!("parsed command and mention: {:?} {:?}", cmd, mention);

    match (&cmd, mention) {
        (emote, Some(mention)) if log_message_repo.contains_emote(emote) => {
            debug!("emote with mention");
            let messages = log_message_repo.messages(emote)?;
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
            let target_name = match determine_mention(&msg, &context).await {
                Some(n) => n,
                None => mention.to_string(),
            };
            let target = Character::new_from_string(target_name.clone(), Gender::Male, true, false);
            trace!("message target: {:?}", target);
            let answers = LogMessageAnswers::new(origin, target)?;
            let condition_texts = extract_condition_texts(&messages.en.targeted)?;
            Ok(Some((condition_texts, answers, target_name)))
        }
        (emote, None) if log_message_repo.contains_emote(emote) => {
            debug!("emote without mention");
            let messages = log_message_repo.messages(emote)?;
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
            let answers = LogMessageAnswers::new(origin, UNTARGETED_TARGET)?;
            let condition_texts = extract_condition_texts(&messages.en.untargeted)?;
            Ok(Some((
                condition_texts,
                answers,
                UNTARGETED_TARGET.name.into_owned(),
            )))
        }
        (emote, _) => Err(HandlerError::UnrecognizedEmote(emote.to_string())),
    }
}

async fn determine_mention(msg: &Message, context: &Context) -> Option<String> {
    if let Some(user) = msg.mentions.first() {
        trace!("mention appears to be a user");
        user.nick_in(context, msg.guild_id?)
            .await
            .or(Some(user.name.clone()))
    } else if let Some(role_id) = msg.mention_roles.first() {
        trace!("mention appears to be a role");
        let role = msg
            .guild(context.cache.clone())?
            .roles
            .get(role_id)?
            .name
            .clone();
        Some(format!("every {} in sight", role))
    } else if msg.mention_everyone {
        trace!("mention appears to be everyone");
        Some("everyone in the vicinity".to_string())
    } else {
        trace!("no mention found");
        None
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, context: Context, msg: Message) {
        trace!("incoming message: {:?}", msg);
        if !msg.is_own(&context) && msg.content.starts_with(PREFIX) {
            let mparts: Vec<_> = msg.content[1..].split_whitespace().collect();
            debug!("message parts: {:?}", mparts);
            if let Some((condition_texts, answers, target_name)) =
                match process_input(&mparts, &self.log_message_repo, &context, &msg).await {
                    Ok(v) => v,
                    Err(err) => {
                        error!("error during processing: {:?}", err);
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
            {
                let mut msg_builder = MessageBuilder::new();
                condition_texts.for_each_texts(&answers, |text| {
                    match text {
                        Text::Dynamic(d) => match d {
                            DynamicText::NpcOriginName
                            | DynamicText::PlayerOriginNameEn
                            | DynamicText::PlayerOriginNameJp => msg_builder.mention(&msg.author),
                            DynamicText::NpcTargetName
                            | DynamicText::PlayerTargetNameEn
                            | DynamicText::PlayerTargetNameJp => msg_builder.push(&target_name),
                        },
                        Text::Static(s) => msg_builder.push(s),
                    };
                });
                if let Err(e) = msg.reply(&context, msg_builder.build()).await {
                    error!("failed to send emote message: {:?}", e);
                    return;
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("expected DISCORD_TOKEN env var");
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let log_message_repo = LogMessageRepository::from_xivapi(None)
        .await
        .expect("couldn't load log message data from xivapi");
    info!(
        "repo initialized with emotes: {:?}",
        log_message_repo.emote_list_by_id().collect::<Vec<_>>()
    );
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler { log_message_repo })
        .await
        .expect("error creating client");

    client.start().await.expect("couldn't start client");
}
