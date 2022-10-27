mod commands;
mod db;
pub mod util;

use commands::CommandsEnum;
use db::{
    models::{DbGuild, DbUser},
    Db,
};
use futures::future::{try_join_all, TryFutureExt};
use sqlx::PgPool;
use std::{borrow::Cow, time::Duration};
use thiserror::Error;
use tokio::sync::OnceCell;
use tracing::*;

use serenity::{
    async_trait,
    model::prelude::{
        command::Command,
        interaction::{application_command::ApplicationCommandInteraction, Interaction},
        Mention, Message, Ready,
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

#[derive(Debug, Clone)]
pub struct MessageDbData<'a> {
    db: &'a Db,
    user_discord_id: String,
    guild_discord_id: Option<String>,
    user_cell: OnceCell<Option<DbUser>>,
    guild_cell: OnceCell<Option<DbGuild>>,
}

impl<'a> MessageDbData<'a> {
    pub fn new(
        db: &Db,
        user_discord_id: String,
        guild_discord_id: Option<String>,
    ) -> MessageDbData {
        MessageDbData {
            db,
            user_discord_id,
            guild_discord_id,
            user_cell: OnceCell::new(),
            guild_cell: OnceCell::new(),
        }
    }

    #[instrument]
    pub async fn user(&self) -> Result<Option<Cow<DbUser>>, HandlerError> {
        Ok(self
            .user_cell
            .get_or_try_init(|| async { self.db.find_user(&self.user_discord_id).await })
            .await?
            .as_ref()
            .map(Cow::Borrowed))
    }

    #[instrument]
    pub async fn guild(&self) -> Result<Option<Cow<DbGuild>>, HandlerError> {
        if let Some(discord_id) = &self.guild_discord_id {
            Ok(self
                .guild_cell
                .get_or_try_init(|| async { self.db.find_guild(discord_id).await })
                .await?
                .as_ref()
                .map(Cow::Borrowed))
        } else {
            Err(HandlerError::NotGuild)
        }
    }

    #[instrument]
    pub async fn determine_user_settings(&self) -> Result<Cow<DbUser>, HandlerError> {
        if let Some(user) = self.user().await? {
            return Ok(user);
        }
        if let Some(guild) = self.guild().await? {
            return Ok(Cow::Owned(DbUser {
                discord_id: self.user_discord_id.clone(),
                ..DbUser::from(guild.as_ref())
            }));
        }
        Ok(Cow::Owned(DbUser::default()))
    }
}

// untargeted messages shouldn't reference target character at all, but just in case
pub const UNTARGETED_TARGET: Character =
    Character::new("Godbert Manderville", Gender::Male, false, false);
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
}

impl HandlerError {
    fn should_followup(&self) -> bool {
        !matches!(self, HandlerError::TimeoutOrOverLimit)
    }
}

#[async_trait]
impl EventHandler for Handler {
    #[instrument(skip(self, context))]
    async fn message(&self, context: Context, msg: Message) {
        async fn handle_error(err: HandlerError, msg: Message, context: &Context) {
            error!("error during message processing: {:?}", err);
            if err.should_followup() {
                if let Err(e) = msg.reply(context, err.to_string()).await {
                    error!(
                        "could not send follow-up message ({}): {:?}",
                        err.to_string(),
                        e
                    );
                }
            }
        }

        if msg.is_own(&context) {
            return;
        }

        let message_db_data = MessageDbData::new(
            &self.db,
            msg.author.id.to_string(),
            msg.guild_id.as_ref().map(ToString::to_string),
        );

        let guild = match message_db_data.guild().await {
            Ok(guild) => guild.unwrap_or_default(),
            Err(e) => {
                error!("error communicating with db: {:?}", e);
                handle_error(e, msg, &context).await;
                return;
            }
        };
        if msg.content.starts_with(&guild.prefix) {
            let mut mparts: Vec<_> = msg.content.split_whitespace().collect();
            if let Some(first) = mparts.get_mut(0) {
                *first = first.strip_prefix(&guild.prefix).unwrap_or(first);
            }
            debug!("message parts: {:?}", mparts);
            match self
                .process_input(&context, &mparts, &msg, &message_db_data)
                .await
            {
                Ok(v) => v,
                Err(err) => {
                    handle_error(err, msg, &context).await;
                }
            }
        }
    }

    #[instrument(skip(self, context))]
    async fn interaction_create(&self, context: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(cmd) = interaction {
            let message_db_data = MessageDbData::new(
                &self.db,
                cmd.user.id.to_string(),
                cmd.guild_id.as_ref().map(ToString::to_string),
            );

            if let Err(err) | Ok(Err(err)) = self
                .try_handle_commands::<GlobalCommands>(&context, &cmd, &message_db_data)
                .or_else(|_| {
                    self.try_handle_commands::<GuildCommands>(&context, &cmd, &message_db_data)
                })
                .await
            {
                error!("error during interaction processing: {:?}", err);
                if err.should_followup() {
                    if let Err(e) = cmd
                        .create_followup_message(&context, |msg| {
                            msg.ephemeral(true).content(err.to_string())
                        })
                        .await
                    {
                        error!(
                            "could not send follow-up message ({}): {:?}",
                            err.to_string(),
                            e
                        );
                    }
                }
            };
        }
    }

    #[instrument(skip(self, context))]
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
            context.shard.shutdown_clean();
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
            context.shard.shutdown_clean();
        }
    }
}

impl Handler {
    #[instrument(skip(self))]
    pub async fn build_emote_message<'a, T: Mentionable + std::fmt::Debug>(
        &self,
        emote: &str,
        message_db_data: &MessageDbData<'a>,
        author_mentionable: &T,
        target: Option<&str>,
    ) -> Result<String, HandlerError> {
        enum BuilderAction<'a> {
            Mention(Mention),
            Text(Cow<'a, str>),
        }

        impl<'a> BuilderAction<'a> {
            fn do_action(self, msg_builder: &mut MessageBuilder) {
                match self {
                    BuilderAction::Mention(m) => msg_builder.mention(&m),
                    BuilderAction::Text(s) => msg_builder.push(s),
                };
            }
        }

        let author_mention = author_mentionable.mention();

        let user = message_db_data.determine_user_settings().await?;
        let DbUser {
            language, gender, ..
        } = user.as_ref();

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

        Ok(condition_texts
            .into_map_texts(&answers, move |text| match text {
                Text::Dynamic(d) => match d {
                    DynamicText::NpcOriginName
                    | DynamicText::PlayerOriginNameEn
                    | DynamicText::PlayerOriginNameJp => Ok(BuilderAction::Mention(author_mention)),
                    DynamicText::NpcTargetName
                    | DynamicText::PlayerTargetNameEn
                    | DynamicText::PlayerTargetNameJp => match &target {
                        Some(t) => Ok(BuilderAction::Text(Cow::Borrowed(t))),
                        None => Err(HandlerError::TargetNone),
                    },
                },
                Text::Static(s) => Ok(BuilderAction::Text(Cow::Owned(s))),
            })
            .fold(Ok(MessageBuilder::new()), |builder_res, action_res| match (
                builder_res,
                action_res,
            ) {
                (Err(e), _) | (_, Err(e)) => Err(e),
                (Ok(mut builder), Ok(action)) => {
                    action.do_action(&mut builder);
                    Ok(builder)
                }
            })?
            .build())
    }

    #[instrument(skip(self, context))]
    async fn process_input<'a>(
        &self,
        context: &Context,
        mparts: &[&str],
        msg: &Message,
        message_db_data: &MessageDbData<'a>,
    ) -> Result<(), HandlerError> {
        let (original_emote, mention) = mparts.split_first().ok_or(HandlerError::EmptyCommand)?;
        let emote = ["/", original_emote].concat();
        let mention = if mention.is_empty() {
            None
        } else {
            Some(mention.join(" "))
        };

        trace!("parsed command and mention: {:?} {:?}", emote, mention);

        match (&emote, mention) {
            (emote, mention_opt) if self.log_message_repo.contains_emote(emote) => {
                debug!("emote, mention: {}", mention_opt.is_some());
                let body = self
                    .build_emote_message(
                        emote,
                        message_db_data,
                        &msg.author,
                        mention_opt.as_ref().map(AsRef::as_ref),
                    )
                    .await?;
                msg.reply(context, body).await?;
                Ok(())
            }
            (_, _) => Err(HandlerError::UnrecognizedEmote(original_emote.to_string())),
        }
    }

    #[instrument(skip(self, context))]
    async fn try_handle_commands<'a, T>(
        &self,
        context: &Context,
        cmd: &ApplicationCommandInteraction,
        message_db_data: &MessageDbData<'a>,
    ) -> Result<Result<(), HandlerError>, HandlerError>
    where
        T: CommandsEnum,
    {
        if let Ok(app_cmd) = T::from_str(cmd.data.name.as_str()) {
            trace!("handing off to app command handler");
            Ok(app_cmd.handle(cmd, self, context, message_db_data).await)
        } else {
            Err(HandlerError::UnrecognizedCommand(cmd.data.name.to_string()))
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
    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.expect("couldn't run migrations");
    info!("executed {} migrations", migrator.migrations.len());
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
