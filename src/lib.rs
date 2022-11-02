mod commands;
mod db;
pub mod handler;
pub mod util;

use commands::CommandsEnum;
use db::{
    models::{DbGuild, DbUser},
    util::DiscordIdExt,
    Db,
};
use futures::{future::try_join_all, stream, StreamExt, TryStreamExt};
use handler::{Handler, HandlerError};
use sqlx::PgPool;
use std::{borrow::Cow, collections::HashMap, fmt::Debug, time::Duration};
use tokio::sync::OnceCell;
use tracing::*;

use serenity::{
    async_trait,
    model::prelude::{
        command::Command,
        interaction::{application_command::ApplicationCommandInteraction, Interaction},
        GuildId, Message, Ready, UserId,
    },
    prelude::{Context, EventHandler, GatewayIntents},
    Client,
};

use crate::commands::{global::GlobalCommands, guild::GuildCommands};

#[derive(Debug, Clone)]
pub struct MessageDbData<'a> {
    db: &'a Db,
    user_discord_id: UserId,
    guild_discord_id: Option<GuildId>,
    user_cell: OnceCell<Option<DbUser>>,
    guild_cell: OnceCell<Option<DbGuild>>,
}

impl<'a> MessageDbData<'a> {
    #[instrument(ret)]
    pub fn new(
        db: &Db,
        user_discord_id: UserId,
        guild_discord_id: Option<GuildId>,
    ) -> MessageDbData {
        MessageDbData {
            db,
            user_discord_id,
            guild_discord_id,
            user_cell: OnceCell::new(),
            guild_cell: OnceCell::new(),
        }
    }

    pub async fn user(&self) -> Result<Option<Cow<DbUser>>, HandlerError> {
        Ok(self
            .user_cell
            .get_or_try_init(|| async { self.db.find_user(&self.user_discord_id).await })
            .await?
            .as_ref()
            .map(Cow::Borrowed))
    }

    pub async fn guild(&self) -> Result<Option<Cow<DbGuild>>, HandlerError> {
        if let Some(discord_id) = &self.guild_discord_id {
            Ok(self
                .guild_cell
                .get_or_try_init(|| async { self.db.find_guild(discord_id).await })
                .await?
                .as_ref()
                .map(Cow::Borrowed))
        } else {
            Ok(None)
        }
    }

    pub async fn determine_user_settings(&self) -> Result<Cow<DbUser>, HandlerError> {
        if let Some(user) = self.user().await? {
            return Ok(user);
        }
        if let Some(guild) = self.guild().await? {
            return Ok(Cow::Owned(DbUser {
                discord_id: self.user_discord_id.to_db_string(),
                ..DbUser::from(guild.as_ref())
            }));
        }
        Ok(Cow::Owned(DbUser::default()))
    }
}

const INTERACTION_TIMEOUT: Duration = Duration::from_secs(60);

#[async_trait]
impl EventHandler for Handler {
    #[instrument(skip(self, context))]
    async fn message(&self, context: Context, msg: Message) {
        async fn handle_error(err: HandlerError, msg: Message, context: &Context) {
            error!(?err, "error during message processing");
            if err.should_followup() {
                if let Err(e) = msg.reply(context, err.to_string()).await {
                    error!(
                        err = ?e,
                        "could not send follow-up message",
                    );
                }
            }
        }

        if msg.is_own(&context) {
            return;
        }

        info!("handling message");

        let message_db_data = MessageDbData::new(&self.db, msg.author.id, msg.guild_id);

        let guild = match message_db_data.guild().await {
            Ok(guild) => guild.unwrap_or_default(),
            Err(HandlerError::NotGuild) => Cow::Owned(DbGuild::default()),
            Err(err) => {
                error!(?err, "error communicating with db");
                handle_error(err, msg, &context).await;
                return;
            }
        };
        debug!(guild.prefix, "using guild prefix");
        if msg.content.starts_with(&guild.prefix) {
            let mut mparts: Vec<_> = msg.content.split_whitespace().collect();
            if let Some(first) = mparts.get_mut(0) {
                *first = first.strip_prefix(&guild.prefix).unwrap_or(first);
            }
            debug!(?mparts);
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
            let message_db_data = MessageDbData::new(&self.db, cmd.user.id, cmd.guild_id);

            let handle_res = match self
                .try_handle_commands::<GlobalCommands>(&context, &cmd, &message_db_data)
                .await
            {
                Some(res) => res,
                None => match self
                    .try_handle_commands::<GuildCommands>(&context, &cmd, &message_db_data)
                    .await
                {
                    Some(r) => r,
                    None => Err(HandlerError::UnrecognizedCommand(cmd.data.name.to_string())),
                },
            };

            if let Err(err) = handle_res {
                error!(?err, "error during interaction processing");
                if err.should_followup() {
                    if let Err(e) = cmd
                        .create_followup_message(&context, |msg| {
                            msg.ephemeral(true).content(err.to_string())
                        })
                        .await
                    {
                        error!(
                            err = ?e,
                            "could not send follow-up message",
                        );
                    }
                }
            };
        }
    }

    #[instrument(skip(self, context))]
    async fn ready(&self, context: Context, ready: Ready) {
        async fn save_command_ids<T>(
            context: &Context,
            commands: impl Iterator<Item = Command>,
        ) -> Result<(), HandlerError>
        where
            T: CommandsEnum,
        {
            let mut cmd_map = HashMap::new();
            for cmd in commands {
                let cmd_enum =
                    T::from_str(&cmd.name).map_err(|_| HandlerError::CommandRegisterUnknown)?;
                if let Some(prev) = cmd_map.insert(cmd.id, cmd_enum) {
                    warn!(?prev, "overwrote previous command with same id");
                }
            }
            context.data.write().await.insert::<T>(cmd_map);
            Ok(())
        }

        info!("{} is connected", ready.user.name);

        info!(
            guilds = ?ready.guilds.iter().map(|ug| ug.id).collect::<Vec<_>>()
        );
        // global commands

        let global_commands = match Command::set_global_application_commands(&context, |create| {
            create.set_application_commands(GlobalCommands::application_commands().collect());
            create
        })
        .await
        {
            Err(err) => {
                error!(?err, "error registering global application commands");
                context.shard.shutdown_clean();
                return;
            }
            Ok(commands) => commands,
        };

        info!(
            commands = ?global_commands.iter().map(|c| &c.name).collect::<Vec<_>>(),
            "registered global commands"
        );
        if let Err(err) =
            save_command_ids::<GlobalCommands>(&context, global_commands.into_iter()).await
        {
            error!(?err, "error saving global application command data");
            context.shard.shutdown_clean();
            return;
        }

        // guild commands

        if !ready.guilds.is_empty() {
            let guild_commands = match try_join_all(ready.guilds.iter().map(|g| {
                g.id.set_application_commands(&context, |create| {
                    create
                        .set_application_commands(GuildCommands::application_commands().collect());
                    create
                })
            }))
            .await
            {
                Err(err) => {
                    error!(?err, "error registering guild application commands");
                    context.shard.shutdown_clean();
                    return;
                }
                Ok(commands) => commands,
            };

            if let Some(first) = guild_commands.first() {
                info!(
                    commands = ?first.iter().map(|c| &c.name).collect::<Vec<_>>(),
                    "registered guild commands"
                );
            } else {
                error!("guilds list is not empty, but no guild commands were registered");
                context.shard.shutdown_clean();
                return;
            }
            if let Err(err) = stream::iter(guild_commands.into_iter())
                .map(Ok)
                .try_for_each(|cmds| async {
                    save_command_ids::<GuildCommands>(&context, cmds.into_iter()).await
                })
                .await
            {
                error!(?err, "error saving guild application command data");
                context.shard.shutdown_clean();
                return;
            }
        }
    }
}

impl Handler {
    #[instrument(skip(self, context, msg))]
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

        debug!(emote, ?mention, "parsed message");

        let emote = self.get_emote_data(&emote);

        match (emote, mention) {
            (Some(emote), mention_opt) => {
                let body = self
                    .build_emote_message(
                        emote,
                        message_db_data,
                        &msg.author,
                        mention_opt.as_ref().map(AsRef::as_ref),
                    )
                    .await?;
                debug!(body, "emote result");
                msg.reply(context, body).await?;
                self.log_emote(
                    &msg.author.id,
                    msg.guild_id.as_ref(),
                    msg.mentions.iter().map(|u| &u.id),
                    emote,
                )
                .await?;
                Ok(())
            }
            (_, _) => {
                warn!("could not find matching emote");
                Err(HandlerError::UnrecognizedEmote(original_emote.to_string()))
            }
        }
    }

    #[instrument(skip_all)]
    async fn try_handle_commands<'a, T>(
        &self,
        context: &Context,
        cmd: &ApplicationCommandInteraction,
        message_db_data: &MessageDbData<'a>,
    ) -> Option<Result<(), HandlerError>>
    where
        T: CommandsEnum,
    {
        let read = context.data.read().await;
        if let Some(cmd_map) = read.get::<T>() {
            if let Some(app_cmd) = cmd_map.get(&cmd.data.id) {
                trace!(?app_cmd, "handing off to app command handler");
                Some(app_cmd.handle(cmd, self, context, message_db_data).await)
            } else {
                None
            }
        } else {
            Some(Err(HandlerError::TypeMapNotFound))
        }
    }
}

pub async fn setup_client(token: String, pool: PgPool) -> Client {
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_MEMBERS;
    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.expect("couldn't run migrations");
    info!("executed {} migrations", migrator.migrations.len());

    let db = Db(pool);

    let handler = Handler::new(db, None).expect("couldn't load log message data from xivapi");
    info!(
        emotes = ?handler.emote_list_by_id().collect::<Vec<_>>(),
        "repo initialized with emotes"
    );

    handler
        .upsert_emotes()
        .await
        .expect("couldn't insert emote data into db");

    Client::builder(&token, intents)
        .event_handler(handler)
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
