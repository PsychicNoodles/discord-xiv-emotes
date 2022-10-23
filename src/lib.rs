mod commands;
mod db;
pub mod util;

use commands::CommandsEnum;
use db::{
    models::{DbGuild, DbUser},
    Db,
};
use futures::future::{try_join_all, TryFutureExt};
use log::*;
use sqlx::PgPool;
use std::time::Duration;
use thiserror::Error;

use serenity::{
    async_trait,
    model::prelude::{
        command::Command,
        interaction::{application_command::ApplicationCommandInteraction, Interaction},
        Message, Ready,
    },
    prelude::Mentionable,
    prelude::{Context, EventHandler, GatewayIntents},
    utils::MessageBuilder,
    Client,
};
use xiv_emote_parser::{
    log_message::{
        condition::{Character, DynamicText, Gender, LogMessageAnswersError},
        parser::{extract_condition_texts, Text},
        EmoteTextError, LogMessageAnswers,
    },
    repository::{LogMessageRepository, LogMessageRepositoryError},
};

use crate::commands::{global::GlobalCommands, guild::GuildCommands};

pub struct Handler {
    log_message_repo: LogMessageRepository,
    db: Db,
}

// untargeted messages shouldn't reference target character at all, but just in case
pub const UNTARGETED_TARGET: Character =
    Character::new("Godbert Manderville", Gender::Male, false, false);
const PREFIX: &str = "!";
const INTERACTION_TIMEOUT: Duration = Duration::from_secs(60);

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
    #[error("Internal error")]
    TypeMapNotFound,
    #[error("Maximum number of commands reached")]
    ApplicationCommandCap,
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

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, context: Context, msg: Message) {
        trace!("incoming message: {:?}", msg);
        if !msg.is_own(&context) && msg.content.starts_with(PREFIX) {
            let mut mparts: Vec<_> = msg.content.split_whitespace().collect();
            if let Some(first) = mparts.get_mut(0) {
                *first = first.strip_prefix(PREFIX).unwrap_or(first);
            }
            debug!("message parts: {:?}", mparts);
            match self.process_input(&context, &mparts, &msg).await {
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

            if let Err(err) = self
                .try_handle_commands::<GlobalCommands>(&context, &cmd)
                .or_else(|_| self.try_handle_commands::<GuildCommands>(&context, &cmd))
                .await
            {
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

        info!(
            "guilds: {:?}",
            ready.guilds.iter().map(|ug| ug.id).collect::<Vec<_>>()
        );

        if let Err(err) = Command::set_global_application_commands(&context, |create| {
            create.set_application_commands(GlobalCommands::application_commands().collect());
            create
        })
        .await
        {
            error!("error registering global application commands: {:?}", err);
        }

        if let Err(err) = try_join_all(ready.guilds.iter().map(|g| {
            g.id.set_application_commands(&context, |create| {
                create.set_application_commands(GuildCommands::application_commands().collect());
                create
            })
        }))
        .await
        {
            error!("error registering guild application commands: {:?}", err);
        }
    }
}

impl Handler {
    pub fn build_emote_message(
        &self,
        emote: &str,
        author_user: Option<DbUser>,
        author_mention: &impl Mentionable,
        target: Option<&str>,
        guild: Option<DbGuild>,
    ) -> Result<String, HandlerError> {
        let mut msg_builder = MessageBuilder::new();

        let (language, gender) = match (&author_user, &guild) {
            (Some(a), _) => (a.language, a.gender),
            (_, Some(g)) => (g.language, g.gender),
            _ => (Default::default(), Default::default()),
        };

        let messages = self.log_message_repo.messages(emote)?;
        let localized_messages = language.with_emote_data(messages);
        let condition_texts = extract_condition_texts(if target.is_some() {
            &localized_messages.targeted
        } else {
            &localized_messages.untargeted
        })?;

        let answers = LogMessageAnswers::new(
            Character::new_from_string(
                author_mention.mention().to_string(),
                gender.into(),
                true,
                false,
            ),
            target
                .as_ref()
                .map(|t| Character::new_from_string(t.to_string(), Gender::Male, true, false))
                .unwrap_or(UNTARGETED_TARGET),
        )?;

        let mut errs: Vec<_> = condition_texts
            .map_texts_mut(&answers, |text| {
                match text {
                    Text::Dynamic(d) => match d {
                        DynamicText::NpcOriginName
                        | DynamicText::PlayerOriginNameEn
                        | DynamicText::PlayerOriginNameJp => msg_builder.mention(author_mention),
                        DynamicText::NpcTargetName
                        | DynamicText::PlayerTargetNameEn
                        | DynamicText::PlayerTargetNameJp => match &target {
                            Some(t) => msg_builder.push(t),
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
        Ok(msg_builder.build())
    }

    async fn process_input(
        &self,
        context: &Context,
        mparts: &[&str],
        msg: &Message,
    ) -> Result<(), HandlerError> {
        let (original_emote, mention) = mparts.split_first().ok_or(HandlerError::EmptyCommand)?;
        let emote = ["/", original_emote].concat();
        let mention = if mention.is_empty() {
            None
        } else {
            Some(mention.join(" "))
        };

        trace!("parsed command and mention: {:?} {:?}", emote, mention);

        let user = self.db.find_user(msg.author.id).await?;
        trace!("user settings: {:?}", user);

        match (&emote, mention) {
            (emote, Some(mention)) if self.log_message_repo.contains_emote(emote) => {
                debug!("emote with mention");
                let guild = if let Some(guild_id) = msg.guild_id {
                    self.db.find_guild(guild_id).await?
                } else {
                    None
                };
                let body =
                    self.build_emote_message(emote, user, &msg.author, Some(&mention), guild)?;
                msg.reply(context, body).await?;
                Ok(())
            }
            (emote, None) if self.log_message_repo.contains_emote(emote) => {
                debug!("emote without mention");
                let guild = if let Some(guild_id) = msg.guild_id {
                    self.db.find_guild(guild_id).await?
                } else {
                    None
                };
                let body = self.build_emote_message(emote, user, &msg.author, None, guild)?;
                msg.reply(context, body).await?;
                Ok(())
            }
            (_, _) => Err(HandlerError::UnrecognizedEmote(original_emote.to_string())),
        }
    }

    async fn try_handle_commands<T>(
        &self,
        context: &Context,
        cmd: &ApplicationCommandInteraction,
    ) -> Result<(), HandlerError>
    where
        T: CommandsEnum,
    {
        match T::from_str(cmd.data.name.as_str()) {
            Ok(app_cmd) => app_cmd.handle(cmd, self, context).await,
            Err(_) => Err(HandlerError::UnrecognizedCommand(cmd.data.name.clone())),
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
