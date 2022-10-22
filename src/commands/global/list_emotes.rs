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

use crate::{commands::AppCmd, Handler, HandlerError};

use super::GlobalCommands;

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
        cmd.name(GlobalCommands::ListEmotes)
            .kind(CommandType::ChatInput)
            .description("List all available emotes")
            .dm_permission(true);
        cmd
    }

    async fn handle(
        cmd: &ApplicationCommandInteraction,
        handler: &Handler,
        context: &Context,
    ) -> Result<(), HandlerError>
    where
        Self: Sized,
    {
        const EMOTE_LIST_PREFIX: &str = "List of emotes";
        let bodies = split_by_max_message_len(
            EMOTE_LIST_PREFIX,
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
}
