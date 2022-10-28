//! Shared data between global and guild versions of the stats command

use serenity::{
    model::prelude::{GuildId, UserId},
    prelude::Mentionable,
};

use crate::{
    db::models::{DbLanguage, DbUser},
    util::LocalizedString,
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
    ja: "検査するユーザー",
};
pub const RECEIVED_GROUP_NAME: LocalizedString = LocalizedString {
    en: "received",
    ja: "受信",
};
pub const RECEIVED_GROUP_DESC: LocalizedString = LocalizedString {
    en: "Message statistics by targets received",
    ja: "ターゲット受信される側の使用統計",
};

#[derive(Debug, Clone)]
pub enum EmoteLogQuery {
    Guild(GuildId),
    GuildUser((GuildId, UserId)),
    User(UserId),
    ReceivedGuild(GuildId),
    ReceivedGuildUser((GuildId, UserId)),
    ReceivedUser(UserId),
}

impl EmoteLogQuery {
    pub fn to_message(&self, count: i64, user: &DbUser) -> String {
        match user.language {
            DbLanguage::En => self.to_en_message(count),
            DbLanguage::Ja => self.to_ja_message(count),
        }
    }

    pub fn to_en_message(&self, count: i64) -> String {
        match self {
            EmoteLogQuery::Guild(_) => {
                format!(
                    "There have been {} emotes sent thus far in this guild!",
                    count
                )
            }
            EmoteLogQuery::GuildUser((_, u)) => {
                format!(
                    "There have been {} emotes sent by {} thus far in this guild!",
                    count,
                    u.mention()
                )
            }
            EmoteLogQuery::User(u) => {
                format!(
                    "There have been {} emotes sent by {} thus far!",
                    count,
                    u.mention()
                )
            }
            EmoteLogQuery::ReceivedGuild(_) => {
                format!(
                    "There have been {} emotes received thus far in this guild!",
                    count
                )
            }
            EmoteLogQuery::ReceivedGuildUser((_, u)) => {
                format!(
                    "There have been {} emotes received by {} thus far in this guild!",
                    count,
                    u.mention()
                )
            }
            EmoteLogQuery::ReceivedUser(u) => {
                format!(
                    "There have been {} emotes received by {} thus far!",
                    count,
                    u.mention()
                )
            }
        }
    }

    pub fn to_ja_message(&self, count: i64) -> String {
        match self {
            EmoteLogQuery::Guild(_) => {
                format!(
                    "今までこのサーバーで{}件のエモートが送信されています！",
                    count
                )
            }
            EmoteLogQuery::GuildUser((_, u)) => {
                format!(
                    "今までこのサーバーで{}が{}件のエモートを送信されています！",
                    u, count
                )
            }
            EmoteLogQuery::User(u) => {
                format!("今まで{}が{}件のエモートを送信されています！", u, count)
            }
            EmoteLogQuery::ReceivedGuild(_) => {
                format!(
                    "今までこのサーバーで{}件のエモートが受信されています！",
                    count
                )
            }
            EmoteLogQuery::ReceivedGuildUser((_, u)) => {
                format!(
                    "今までこのサーバーで{}が{}件のエモートを受信されています！",
                    u, count
                )
            }
            EmoteLogQuery::ReceivedUser(u) => {
                format!("今まで{}が{}件のエモートを受信されています！", u, count)
            }
        }
    }
}
