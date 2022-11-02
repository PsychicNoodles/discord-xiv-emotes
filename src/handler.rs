pub mod commands;
pub mod emotes;

use std::{collections::HashMap, sync::Arc};
use thiserror::Error;
use tracing::*;

use xiv_emote_parser::{
    log_message::{
        condition::LogMessageAnswersError,
        parser::{extract_condition_texts, ConditionTexts},
        EmoteTextError,
    },
    repository::{xivapi, LogMessageRepository, LogMessageRepositoryError},
};

use crate::db::Db;

#[derive(Debug, Clone)]
pub struct ConditionTextPair {
    pub targeted: ConditionTexts,
    pub untargeted: ConditionTexts,
}

#[derive(Debug, Clone)]
pub struct EmoteData {
    pub id: u32,
    pub name: String,
    pub en: ConditionTextPair,
    pub ja: ConditionTextPair,
}

pub struct Handler {
    pub emotes: HashMap<String, Arc<EmoteData>>,
    pub db: Db,
}

impl Handler {
    #[instrument(level = "trace")]
    pub fn new(db: Db, api_key: Option<String>) -> Result<Handler, HandlerError> {
        let query = LogMessageRepository::prep_xivapi_query(api_key);
        let emotes = LogMessageRepository::load_xivapi(&query)?
            .into_iter()
            .try_fold(
                HashMap::new(),
                |mut map, result| -> Result<_, HandlerError> {
                    trace!("processing from xivapi: {:?}", result);
                    if let xivapi::EmoteData {
                        log_message_targeted: Some(targeted),
                        log_message_untargeted: Some(untargeted),
                        text_command: Some(text_command),
                        name: Some(name),
                        id: Some(id),
                    } = result
                    {
                        if [
                            &targeted.text_en,
                            &untargeted.text_en,
                            &targeted.text_ja,
                            &untargeted.text_ja,
                        ]
                        .into_iter()
                        .any(String::is_empty)
                        {
                            trace!(name, id, "skipping due to no messages");
                            return Ok(map);
                        }
                        let en_targeted =
                            extract_condition_texts(&targeted.text_en).map_err(|e| {
                                error!(
                                    name,
                                    id, "could not extract condition texts for en->targeted"
                                );
                                e
                            })?;
                        let en_untargeted =
                            extract_condition_texts(&untargeted.text_en).map_err(|e| {
                                error!(
                                    name,
                                    id, "could not extract condition texts for en->untargeted"
                                );
                                e
                            })?;
                        let ja_targeted =
                            extract_condition_texts(&targeted.text_ja).map_err(|e| {
                                error!(
                                    name,
                                    id, "could not extract condition texts for ja->targeted"
                                );
                                e
                            })?;
                        let ja_untargeted =
                            extract_condition_texts(&untargeted.text_ja).map_err(|e| {
                                error!(
                                    name,
                                    id, "could not extract condition texts for ja->untargeted"
                                );
                                e
                            })?;
                        let data = Arc::new(EmoteData {
                            id,
                            name,
                            en: ConditionTextPair {
                                targeted: en_targeted,
                                untargeted: en_untargeted,
                            },
                            ja: ConditionTextPair {
                                targeted: ja_targeted,
                                untargeted: ja_untargeted,
                            },
                        });
                        [
                            text_command.alias_en,
                            text_command.alias_ja,
                            text_command.command_en,
                            text_command.command_ja,
                        ]
                        .into_iter()
                        .flatten()
                        .filter(|cmd| !cmd.is_empty())
                        .for_each(|cmd| {
                            trace!("{} => {}", cmd, data.name);
                            map.insert(cmd, data.clone());
                        })
                    } else {
                        trace!("ignoring invalid emote data ({:?})", result);
                    }
                    Ok(map)
                },
            )?;
        Ok(Handler { db, emotes })
    }
}

#[derive(Debug, Error)]
pub enum HandlerError {
    #[error("Unrecognized emote ({0})")]
    UnrecognizedEmote(String),
    #[error("Unrecognized command ({0})")]
    UnrecognizedCommand(String),
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
    Db(#[from] sqlx::Error),
    #[error("Failed to send message")]
    Send(#[from] serenity::Error),
    #[error("Command can only be used in a server")]
    NotGuild,
    #[error("Timed out or had too many inputs")]
    TimeoutOrOverLimit,
    #[error("Couldn't find user")]
    UserNotFound,
    #[error("Unexpected data received from server")]
    UnexpectedData,
    #[error("Maximum number of commands reached")]
    ApplicationCommandCap,
    #[error("Internal error, could not build response")]
    EmoteLogCountNoParams,
    #[error("Internal error, could not build response")]
    CountNone,
    #[error("Received command info for unknown command")]
    CommandRegisterUnknown,
    #[error("Internal error, could not build response")]
    TypeMapNotFound,
    #[error("Could not set up application commands")]
    CommandSetup,
}

impl HandlerError {
    pub fn should_followup(&self) -> bool {
        !matches!(self, HandlerError::TimeoutOrOverLimit)
    }
}
