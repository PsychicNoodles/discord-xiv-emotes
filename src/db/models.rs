use xiv_emote_parser::{
    log_message::condition::Gender,
    repository::{EmoteData, LogMessagePair},
};

use strum_macros::{EnumIter, FromRepr};
use time::OffsetDateTime;

#[derive(sqlx::Type, Default, Debug, Clone, Copy, PartialEq, Eq, EnumIter, FromRepr)]
#[repr(i32)]
pub enum DbLanguage {
    #[default]
    En = 0,
    Ja = 1,
}

impl DbLanguage {
    pub fn to_string_en(self) -> &'static str {
        match self {
            DbLanguage::En => "English",
            DbLanguage::Ja => "Japanese",
        }
    }

    pub fn to_string_ja(self) -> &'static str {
        match self {
            DbLanguage::En => "英語",
            DbLanguage::Ja => "日本語",
        }
    }

    pub fn to_string(self, language: DbLanguage) -> &'static str {
        match language {
            DbLanguage::En => self.to_string_en(),
            DbLanguage::Ja => self.to_string_ja(),
        }
    }

    pub fn with_emote_data<'a>(&'a self, emote_data: &'a EmoteData) -> &LogMessagePair {
        match self {
            DbLanguage::En => &emote_data.en,
            DbLanguage::Ja => &emote_data.ja,
        }
    }

    pub fn for_user(self, user: &DbUser) -> &'static str {
        self.to_string(user.language)
    }
}

#[derive(sqlx::Type, Default, Debug, Clone, Copy, PartialEq, Eq, EnumIter, FromRepr)]
#[repr(i32)]
pub enum DbGender {
    #[default]
    M = 0,
    F = 1,
}

impl DbGender {
    pub fn to_string_en(self) -> &'static str {
        match self {
            DbGender::M => "Male",
            DbGender::F => "Female",
        }
    }

    pub fn to_string_ja(self) -> &'static str {
        match self {
            DbGender::M => "男性",
            DbGender::F => "女性",
        }
    }

    pub fn to_string(self, language: DbLanguage) -> &'static str {
        match language {
            DbLanguage::En => self.to_string_en(),
            DbLanguage::Ja => self.to_string_ja(),
        }
    }

    pub fn for_user(self, user: &DbUser) -> &'static str {
        self.to_string(user.language)
    }
}

impl From<DbGender> for Gender {
    fn from(g: DbGender) -> Self {
        From::from(&g)
    }
}

impl From<&DbGender> for Gender {
    fn from(g: &DbGender) -> Self {
        match g {
            DbGender::M => Gender::Male,
            DbGender::F => Gender::Female,
        }
    }
}

#[derive(sqlx::FromRow, Debug, Clone)]
#[sqlx(type_name = "user")]
pub struct DbUser {
    pub discord_id: String,
    pub language: DbLanguage,
    pub gender: DbGender,
    pub insert_tm: time::OffsetDateTime,
    pub update_tm: time::OffsetDateTime,
}

impl Default for DbUser {
    fn default() -> Self {
        DbUser {
            discord_id: String::default(),
            language: DbLanguage::default(),
            gender: DbGender::default(),
            insert_tm: OffsetDateTime::now_utc(),
            update_tm: OffsetDateTime::now_utc(),
        }
    }
}

impl DbUser {
    pub fn discord_id(&self) -> &String {
        &self.discord_id
    }

    pub fn language(&self) -> &DbLanguage {
        &self.language
    }

    pub fn gender(&self) -> &DbGender {
        &self.gender
    }
}

impl From<DbGuild> for DbUser {
    fn from(g: DbGuild) -> Self {
        Self::from(&g)
    }
}

impl From<&DbGuild> for DbUser {
    fn from(g: &DbGuild) -> Self {
        DbUser {
            language: g.language,
            gender: g.gender,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct DbUserOpt(pub Option<DbUser>);

impl From<DbUserOpt> for Option<DbUser> {
    fn from(o: DbUserOpt) -> Self {
        o.into_inner()
    }
}

impl DbUserOpt {
    pub fn into_inner(self) -> Option<DbUser> {
        self.0
    }

    pub fn language(&self) -> DbLanguage {
        self.0
            .as_ref()
            .map(DbUser::language)
            .cloned()
            .unwrap_or_default()
    }

    pub fn gender(&self) -> DbGender {
        self.0
            .as_ref()
            .map(DbUser::gender)
            .cloned()
            .unwrap_or_default()
    }
}

const DEFAULT_PREFIX: &str = "!";

#[derive(sqlx::FromRow, Debug, Clone)]
#[sqlx(type_name = "guild")]
pub struct DbGuild {
    pub discord_id: String,
    pub language: DbLanguage,
    pub gender: DbGender,
    pub prefix: String,
    pub insert_tm: time::OffsetDateTime,
    pub update_tm: time::OffsetDateTime,
}

impl Default for DbGuild {
    fn default() -> Self {
        DbGuild {
            discord_id: String::default(),
            language: DbLanguage::default(),
            gender: DbGender::default(),
            prefix: DEFAULT_PREFIX.to_string(),
            insert_tm: OffsetDateTime::now_utc(),
            update_tm: OffsetDateTime::now_utc(),
        }
    }
}
