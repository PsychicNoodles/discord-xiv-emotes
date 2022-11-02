//! Shared data between global and guild versions of the stats command

use std::{borrow::Cow, collections::HashMap, sync::Arc};

use serenity::{
    model::prelude::{
        interaction::application_command::{CommandDataOption, CommandDataOptionValue},
        GuildId, UserId,
    },
    utils::MessageBuilder,
};
use tracing::*;

use crate::{
    commands::guild::stats::{RECEIVED_GUILD_SUB_NAME, RECEIVED_GUILD_USER_SUB_NAME},
    db::models::{DbLanguage, DbUser},
    handler::EmoteData,
    util::LocalizedString,
};

use super::{
    global::stats::{RECEIVED_USER_SUB_NAME, USER_SUB_NAME},
    guild::stats::{GUILD_SUB_NAME, GUILD_USER_SUB_NAME},
};
pub const NAME: LocalizedString = LocalizedString {
    en: "stats",
    ja: "統計",
};
pub const DESC: LocalizedString = LocalizedString {
    en: "Emote usage statistics",
    ja: "エモート使用統計",
};
pub const USER_OPT_NAME: LocalizedString = LocalizedString {
    en: "user",
    ja: "ユーザー",
};
pub const USER_OPT_DESC: LocalizedString = LocalizedString {
    en: "User to investigate",
    ja: "ユーザーの絞り込み",
};
pub const RECEIVED_GROUP_NAME: LocalizedString = LocalizedString {
    en: "received",
    ja: "受信",
};
pub const RECEIVED_GROUP_DESC: LocalizedString = LocalizedString {
    en: "Message statistics by targets received",
    ja: "ターゲット受信される側の使用統計",
};
pub const EMOTE_OPT_NAME: LocalizedString = LocalizedString {
    en: "emote",
    ja: "エモート",
};
pub const EMOTE_OPT_DESC: LocalizedString = LocalizedString {
    en: "Emote to filter by",
    ja: "エモートの絞り込み",
};

#[derive(Debug, Clone)]
pub enum EmoteLogQuery {
    Guild((GuildId, Option<Arc<EmoteData>>)),
    GuildUser((GuildId, UserId, Option<Arc<EmoteData>>)),
    User((UserId, Option<Arc<EmoteData>>)),
    ReceivedGuild((GuildId, Option<Arc<EmoteData>>)),
    ReceivedGuildUser((GuildId, UserId, Option<Arc<EmoteData>>)),
    ReceivedUser((UserId, Option<Arc<EmoteData>>)),
}

impl EmoteLogQuery {
    #[instrument(level = "trace")]
    pub fn to_message(&self, count: i64, user: &DbUser) -> String {
        trace!("making stats command message");
        match user.language {
            DbLanguage::En => self.to_en_message(count),
            DbLanguage::Ja => self.to_ja_message(count),
        }
    }

    pub fn to_en_message(&self, count: i64) -> String {
        let mut mb = MessageBuilder::new();
        match self {
            EmoteLogQuery::Guild((_, em_opt)) => {
                mb.push("There have been ").push(count).push(" ");
                if let Some(em) = em_opt {
                    mb.push_mono(&em.name);
                } else {
                    mb.push("emote");
                }
                mb.push("s sent thus far in this guild!").build()
            }
            EmoteLogQuery::GuildUser((_, u, em_opt)) => {
                mb.push("There have been ").push(count).push(" ");
                if let Some(em) = em_opt {
                    mb.push_mono(&em.name);
                } else {
                    mb.push("emote");
                }
                mb.push("s sent by ")
                    .mention(u)
                    .push(" thus far in this guild!")
                    .build()
            }
            EmoteLogQuery::User((u, em_opt)) => {
                mb.push("There have been ").push(count).push(" ");
                if let Some(em) = em_opt {
                    mb.push_mono(&em.name);
                } else {
                    mb.push("emote");
                }
                mb.push("s sent by ").mention(u).push(" thus far!").build()
            }
            EmoteLogQuery::ReceivedGuild((_, em_opt)) => {
                mb.push("There have been ").push(count).push(" ");
                if let Some(em) = em_opt {
                    mb.push_mono(&em.name);
                } else {
                    mb.push("emote");
                }
                mb.push("s received thus far in this guild!").build()
            }
            EmoteLogQuery::ReceivedGuildUser((_, u, em_opt)) => {
                mb.push("There have been ").push(count).push(" ");
                if let Some(em) = em_opt {
                    mb.push_mono(&em.name);
                } else {
                    mb.push("emote");
                }
                mb.push("s received by ")
                    .mention(u)
                    .push(" thus far in this guild!")
                    .build()
            }
            EmoteLogQuery::ReceivedUser((u, em_opt)) => {
                mb.push("There have been ").push(count).push(" ");
                if let Some(em) = em_opt {
                    mb.push_mono(&em.name);
                } else {
                    mb.push("emote");
                }
                mb.push("s received by ")
                    .mention(u)
                    .push(" thus far!")
                    .build()
            }
        }
    }

    pub fn to_ja_message(&self, count: i64) -> String {
        let mut mb = MessageBuilder::new();
        match self {
            EmoteLogQuery::Guild((_, em_opt)) => {
                mb.push("今までこのサーバーで").push(count).push("件の");
                if let Some(em) = em_opt {
                    mb.push_mono(&em.name);
                } else {
                    mb.push("エモート");
                }
                mb.push("が送信されています！").build()
            }
            EmoteLogQuery::GuildUser((_, u, em_opt)) => {
                mb.push("今までこのサーバーで")
                    .mention(u)
                    .push("が")
                    .push(count)
                    .push("件の");
                if let Some(em) = em_opt {
                    mb.push_mono(&em.name);
                } else {
                    mb.push("エモート");
                }
                mb.push("を送信されています！").build()
            }
            EmoteLogQuery::User((u, em_opt)) => {
                mb.push("今まで")
                    .mention(u)
                    .push("が")
                    .push(count)
                    .push("件の");
                if let Some(em) = em_opt {
                    mb.push_mono(&em.name);
                } else {
                    mb.push("エモート");
                }
                mb.push("を送信されています！").build()
            }
            EmoteLogQuery::ReceivedGuild((_, em_opt)) => {
                mb.push("今までこのサーバーで").push(count).push("件の");
                if let Some(em) = em_opt {
                    mb.push_mono(&em.name);
                } else {
                    mb.push("エモート");
                }
                mb.push("が受信されています！").build()
            }
            EmoteLogQuery::ReceivedGuildUser((_, u, em_opt)) => {
                mb.push("今までこのサーバーで")
                    .mention(u)
                    .push("が")
                    .push(count)
                    .push("件の");
                if let Some(em) = em_opt {
                    mb.push_mono(&em.name);
                } else {
                    mb.push("エモート");
                }
                mb.push("を受信されています！").build()
            }
            EmoteLogQuery::ReceivedUser((u, em_opt)) => {
                mb.push("今まで")
                    .mention(u)
                    .push("が")
                    .push(count)
                    .push("件の");
                if let Some(em) = em_opt {
                    mb.push_mono(&em.name);
                } else {
                    mb.push("エモート");
                }
                mb.push("を受信されています！").build()
            }
        }
    }

    #[instrument(skip(emotes))]
    pub fn from_command_data(
        emotes: &HashMap<String, Arc<EmoteData>>,
        options: &[CommandDataOption],
        guild_id_opt: Option<GuildId>,
        user_id_opt: Option<UserId>,
    ) -> Option<EmoteLogQuery> {
        debug!("determining stat command query type");
        fn get_emote_opt(
            emotes: &HashMap<String, Arc<EmoteData>>,
            opt: &CommandDataOption,
            ind: usize,
        ) -> Option<Arc<EmoteData>> {
            let mut emote = match opt.options.get(ind).and_then(|o| o.resolved.as_ref()) {
                Some(CommandDataOptionValue::String(s)) => Some(Cow::Borrowed(s.as_str())),
                Some(v) => {
                    warn!(?v, "resolved to non-string value, ignoring");
                    None
                }
                None => None,
            };
            trace!(?emote, "resolved emote");
            if let Some(e) = emote.as_mut() {
                if !e.starts_with('/') {
                    *e = Cow::Owned(["/", e].concat())
                }
            };
            emote.and_then(|em| emotes.get(em.as_ref()).cloned())
        }

        if let Some(top) = &options.get(0) {
            debug!(?top);
            match (&top.name, guild_id_opt, user_id_opt) {
                // guild
                (_s, Some(guild_id), _) if GUILD_SUB_NAME.any_eq(_s) => Some(EmoteLogQuery::Guild(
                    (guild_id, get_emote_opt(emotes, top, 0)),
                )),
                (_s, Some(guild_id), Some(user_id)) if GUILD_USER_SUB_NAME.any_eq(_s) => Some(
                    EmoteLogQuery::GuildUser((guild_id, user_id, get_emote_opt(emotes, top, 1))),
                ),
                // global
                (_s, _, Some(user_id)) if USER_SUB_NAME.any_eq(_s) => Some(EmoteLogQuery::User((
                    user_id,
                    get_emote_opt(emotes, top, 1),
                ))),
                // received subcommand group
                // everything shifted over, so re-match on guild_id_opt and user_id_opt
                (_s, _, _) if RECEIVED_GROUP_NAME.any_eq(_s) => {
                    if let Some(received) = top.options.get(0) {
                        debug!(?received);
                        match (&received.name, guild_id_opt, user_id_opt) {
                            // guild
                            (_s, Some(guild_id), _) if RECEIVED_GUILD_SUB_NAME.any_eq(_s) => {
                                Some(EmoteLogQuery::ReceivedGuild((
                                    guild_id,
                                    get_emote_opt(emotes, received, 0),
                                )))
                            }
                            (_s, Some(guild_id), Some(user_id))
                                if RECEIVED_GUILD_USER_SUB_NAME.any_eq(_s) =>
                            {
                                Some(EmoteLogQuery::ReceivedGuildUser((
                                    guild_id,
                                    user_id,
                                    get_emote_opt(emotes, received, 1),
                                )))
                            }
                            // global
                            (_s, _, Some(user_id)) if RECEIVED_USER_SUB_NAME.any_eq(_s) => {
                                Some(EmoteLogQuery::ReceivedUser((
                                    user_id,
                                    get_emote_opt(emotes, received, 1),
                                )))
                            }
                            _ => {
                                error!("subcommand group received had invalid sub-values");
                                None
                            }
                        }
                    } else {
                        error!("subcommand group received had no subcommand values");
                        None
                    }
                }
                _ => None,
            }
        } else {
            error!("no top level command data");
            None
        }
    }
}
