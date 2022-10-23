use serenity::builder::{CreateApplicationCommand, CreateApplicationCommandOption};

use crate::db::models::{DbLanguage, DbUser};

pub struct LocalizedString {
    pub en: &'static str,
    pub ja: &'static str,
}

pub trait CreateApplicationCommandExt {
    fn localized_name(&mut self, str: LocalizedString) -> &mut Self;
    fn localized_desc(&mut self, str: LocalizedString) -> &mut Self;
}

impl CreateApplicationCommandExt for CreateApplicationCommand {
    fn localized_name(&mut self, str: LocalizedString) -> &mut Self {
        self.name(str.en)
            .name_localized("en-US", str.en)
            .name_localized("ja", str.ja)
    }

    fn localized_desc(&mut self, str: LocalizedString) -> &mut Self {
        self.description(str.en)
            .description_localized("en-US", str.en)
            .description_localized("ja", str.ja)
    }
}

pub trait CreateApplicationCommandOptionExt {
    fn localized_name(&mut self, str: LocalizedString) -> &mut Self;
    fn localized_desc(&mut self, str: LocalizedString) -> &mut Self;
}

impl CreateApplicationCommandOptionExt for CreateApplicationCommandOption {
    fn localized_name(&mut self, str: LocalizedString) -> &mut Self {
        self.name(str.en)
            .name_localized("en-US", str.en)
            .name_localized("ja", str.ja)
    }

    fn localized_desc(&mut self, str: LocalizedString) -> &mut Self {
        self.description(str.en)
            .description_localized("en-US", str.en)
            .description_localized("ja", str.ja)
    }
}

impl LocalizedString {
    pub fn for_user(&self, user: &DbUser) -> &'static str {
        match user.language {
            DbLanguage::En => self.en,
            DbLanguage::Ja => self.ja,
        }
    }

    pub fn any_eq(&self, str: impl AsRef<str>) -> bool {
        self.en == str.as_ref() || self.ja == str.as_ref()
    }
}
