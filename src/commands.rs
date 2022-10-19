use serenity::builder::CreateApplicationCommand;
use strum_macros::{AsRefStr, Display, EnumIter, EnumString};

pub mod emote_select;
pub mod user_settings;

#[derive(Debug, Clone, AsRefStr, Display, EnumString, EnumIter)]
pub enum Commands {
    #[strum(serialize = "emote")]
    EmoteSelect,
    #[strum(serialize = "settings")]
    UserSettings,
}

impl Commands {
    pub fn register(
        &self,
    ) -> Box<dyn FnOnce(&mut CreateApplicationCommand) -> &mut CreateApplicationCommand> {
        match self {
            Commands::EmoteSelect => Box::new(emote_select::register_chat_input),
            Commands::UserSettings => Box::new(user_settings::register_chat_input),
        }
    }
}
