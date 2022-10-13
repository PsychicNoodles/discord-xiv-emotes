use dotenv::dotenv;
use std::env;

use serenity::{
    async_trait,
    model::prelude::Message,
    prelude::{Context, EventHandler, GatewayIntents},
    utils::MessageBuilder,
    Client,
};
use xiv_emote_parser::{
    log_message::{
        condition::{Character, DynamicText, Gender},
        parser::{extract_condition_texts, Text},
        LogMessageAnswers,
    },
    repository::LogMessageRepository,
};

struct Handler {
    log_message_repo: LogMessageRepository,
}

// untargeted messages shouldn't reference target character at all, but just in case
const UNTARGETED_TARGET: Character =
    Character::new("Godbert Manderville", Gender::Male, false, false);

async fn invalid_message_reply(msg: &Message, context: &Context, mparts: &Vec<&str>, body: &str) {
    eprintln!("invalid message: {:?}", mparts);
    if let Err(e) = msg.reply(context, body).await {
        eprintln!("could not send follow-up message ({}): {:?}", body, e);
    }
}

async fn determine_mention(msg: &Message, context: &Context) -> Option<String> {
    if let Some(user) = msg.mentions.first() {
        user.nick_in(context, msg.guild_id?).await
    } else if let Some(role_id) = msg.mention_roles.first() {
        let role = msg
            .guild(context.cache.clone())?
            .roles
            .get(role_id)?
            .name
            .clone();
        Some(format!("every {} in sight", role))
    } else if msg.mention_everyone {
        Some("everyone in the vicinity".to_string())
    } else {
        None
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, context: Context, msg: Message) {
        if msg.content.starts_with('!') {
            let mparts: Vec<_> = msg.content[1..].split_whitespace().collect();
            let (condition_texts, answers, target_name) = match &mparts[..] {
                [emote, mention] if self.log_message_repo.contains_emote(emote) => {
                    let messages = match self.log_message_repo.messages(emote) {
                        Ok(m) => m,
                        Err(e) => {
                            eprintln!("error retrieving emote messages: {:?}", e);
                            return;
                        }
                    };
                    // todo allow setting gender
                    let origin = Character::new_from_string(
                        msg.author_nick(&context)
                            .await
                            .unwrap_or_else(|| msg.author.name.clone()),
                        Gender::Male,
                        true,
                        false,
                    );
                    let target_name = match determine_mention(&msg, &context).await {
                        Some(n) => n,
                        None => mention.to_string(),
                    };
                    let target =
                        Character::new_from_string(target_name.clone(), Gender::Male, true, false);
                    let answers = match LogMessageAnswers::new(origin, target) {
                        Ok(a) => a,
                        Err(e) => {
                            eprintln!("error building log data: {:?}", e);
                            return;
                        }
                    };
                    let condition_texts = match extract_condition_texts(messages.en_targeted) {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("error building log message: {:?}", e);
                            return;
                        }
                    };
                    (condition_texts, answers, target_name)
                }
                [emote] if self.log_message_repo.contains_emote(emote) => {
                    let messages = match self.log_message_repo.messages(emote) {
                        Ok(m) => m,
                        Err(e) => {
                            eprintln!("error retrieving emote messages: {:?}", e);
                            return;
                        }
                    };
                    // todo allow setting gender
                    let origin = Character::new_from_string(
                        msg.author_nick(&context)
                            .await
                            .unwrap_or_else(|| msg.author.name.clone()),
                        Gender::Male,
                        true,
                        false,
                    );
                    let answers = match LogMessageAnswers::new(origin, UNTARGETED_TARGET) {
                        Ok(a) => a,
                        Err(e) => {
                            eprintln!("error building log message: {:?}", e);
                            return;
                        }
                    };
                    let condition_texts = match extract_condition_texts(messages.en_untargeted) {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("error building log message: {:?}", e);
                            return;
                        }
                    };
                    (
                        condition_texts,
                        answers,
                        UNTARGETED_TARGET.name.into_owned(),
                    )
                }
                [_, _] => {
                    invalid_message_reply(&msg, &context, &mparts, "unrecognized emote").await;
                    return;
                }
                [_] => {
                    invalid_message_reply(&msg, &context, &mparts, "unrecognized emote").await;
                    return;
                }
                _ => {
                    invalid_message_reply(&msg, &context, &mparts, "invalid emote format").await;
                    return;
                }
            };

            let mut msg_builder = MessageBuilder::new();
            condition_texts.for_each_texts(&answers, |text| {
                match text {
                    Text::Dynamic(d) => match d {
                        DynamicText::NpcOriginName
                        | DynamicText::PlayerOriginNameEn
                        | DynamicText::PlayerOriginNameJp => msg_builder.mention(&msg.author),
                        DynamicText::NpcTargetName
                        | DynamicText::PlayerTargetNameEn
                        | DynamicText::PlayerTargetNameJp => msg_builder.push(&target_name),
                    },
                    Text::Static(s) => msg_builder.push(s),
                };
            });
            if let Err(e) = msg.reply(&context, msg_builder.build()).await {
                eprintln!("failed to send emote message: {:?}", e);
                return;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("expected a discord token in the env");
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let log_message_repo = LogMessageRepository::from_xivapi(None)
        .await
        .expect("couldn't load log message data from xivapi");
    println!(
        "repo initialized with emotes: {:?}",
        log_message_repo.emote_list().collect::<Vec<_>>()
    );
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler { log_message_repo })
        .await
        .expect("error creating client");

    if let Err(e) = client.start().await {
        eprintln!("client error: {:?}", e);
    }
}
