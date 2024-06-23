use teloxide::{
    adaptors::{DefaultParseModeRequest, throttle::ThrottlingRequest, DefaultParseMode, Throttle},
    payloads::{EditMessageText, SendDocument, SendDocumentSetters, SendMessage},
    requests::{JsonRequest, MultipartRequest, Requester},
    types::{MessageId, Recipient},
};

use crate::text::Markdown;

// pub type Bot = DefaultParseMode<Throttle<teloxide::Bot>>;
pub type Bot = Throttle<DefaultParseMode<teloxide::Bot>>;

pub trait BotExt {
    fn send_markdown<C, T>(
        &self,
        chat_id: C,
        text: T,
    ) -> ThrottlingRequest<DefaultParseModeRequest<JsonRequest<SendMessage>>>
    where
        C: Into<Recipient>,
        T: Into<Markdown>;

    fn edit_message_markdown<C, T>(
        &self,
        chat_id: C,
        message_id: MessageId,
        text: T,
    ) -> DefaultParseModeRequest<JsonRequest<EditMessageText>>
    where
        C: Into<Recipient>,
        T: Into<Markdown>;
}

impl BotExt for Bot {
    fn send_markdown<C, T>(
        &self,
        chat_id: C,
        text: T, // TODO: this should be Into<Markdown> (todo: create markdown type)
    ) -> ThrottlingRequest<DefaultParseModeRequest<JsonRequest<SendMessage>>>
    where
        C: Into<Recipient>,
        T: Into<Markdown>,
    {
        #[allow(clippy::disallowed_methods)]
        self.send_message(chat_id, text.into())
    }

    fn edit_message_markdown<C, T>(
        &self,
        chat_id: C,
        message_id: MessageId,
        text: T,
    ) -> DefaultParseModeRequest<JsonRequest<EditMessageText>>
    where
        C: Into<Recipient>,
        T: Into<Markdown>,
    {
        #[allow(clippy::disallowed_methods)]
        self.edit_message_text(chat_id, message_id, text.into())
    }
}

pub trait SendDocumentExt {
    fn markdown_caption<T>(self, value: T) -> Self
    where
        T: Into<Markdown>;
}

impl SendDocumentExt for ThrottlingRequest<DefaultParseModeRequest<MultipartRequest<SendDocument>>> {
    fn markdown_caption<T>(self, value: T) -> Self
    where
        T: Into<Markdown>,
    {
        #[allow(clippy::disallowed_methods)]
        self.caption(value.into())
    }
}
