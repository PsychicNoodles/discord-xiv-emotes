use async_trait::async_trait;
use log::*;
use serenity::{
    builder::CreateApplicationCommand,
    constants::MESSAGE_CODE_LIMIT,
    model::prelude::{
        command::CommandType, interaction::application_command::ApplicationCommandInteraction,
    },
    prelude::Context,
};

use crate::{
    commands::AppCmd,
    util::{CreateApplicationCommandExt, LocalizedString},
    Handler, HandlerError, MessageDbData,
};

pub const NAME: LocalizedString = LocalizedString {
    en: "emotes",
    ja: "エモート一覧",
};
pub const DESC: LocalizedString = LocalizedString {
    en: "List all available emotes",
    ja: "選択できるエモートの一覧",
};
pub const LIST_MSG_PREFIX: LocalizedString = LocalizedString {
    en: "List of emotes",
    ja: "エモート一覧",
};

pub struct ListEmotesCmd;

pub fn split_by_max_message_len(
    prefix: impl AsRef<str>,
    mut body: impl Iterator<Item = String>,
) -> Vec<String> {
    let mut res = vec![];
    let mut msg = if let Some(item) = body.next() {
        item
    } else {
        return res;
    };
    for item in body {
        msg.push_str(", ");

        // todo count with codepoints rather than String len?
        if prefix.as_ref().len() + " (xx/xx): ".len() + msg.len() + item.len() + ", ".len()
            > MESSAGE_CODE_LIMIT
        {
            res.push(msg);
            msg = String::new();
        }

        msg.push_str(&item);
    }
    res.push(msg);
    let count = res.len();
    res.iter_mut().enumerate().for_each(|(i, m)| {
        m.insert_str(0, &format!("{} ({}/{}): ", prefix.as_ref(), i + 1, count));
    });
    trace!("res: {:?}", res);
    res
}

#[async_trait]
impl AppCmd for ListEmotesCmd {
    fn to_application_command() -> CreateApplicationCommand
    where
        Self: Sized,
    {
        let mut cmd = CreateApplicationCommand::default();
        cmd.localized_name(NAME)
            .kind(CommandType::ChatInput)
            .localized_desc(DESC)
            .dm_permission(true);
        cmd
    }

    async fn handle(
        cmd: &ApplicationCommandInteraction,
        handler: &Handler,
        context: &Context,
        message_db_data: &MessageDbData,
    ) -> Result<(), HandlerError>
    where
        Self: Sized,
    {
        let user = message_db_data.determine_user_settings().await?;
        let bodies = split_by_max_message_len(
            LIST_MSG_PREFIX.for_user(&user),
            handler.log_message_repo.emote_list_by_id().cloned(),
        );
        debug!("emotes response is {} messages long", bodies.len());

        let mut body_iter = bodies.into_iter();

        if let Some(body) = body_iter.next() {
            cmd.create_interaction_response(context, |res| {
                res.interaction_response_data(|data| data.content(body))
            })
            .await?;
        }

        for body in body_iter {
            cmd.create_followup_message(context, |data| data.content(body))
                .await?;
        }

        Ok(())
    }

    fn name() -> LocalizedString {
        NAME
    }
}
