use yewoh::protocol::{MessageKind, UnicodeTextMessage};
use yewoh::types::FixedString;
use yewoh_server::world::connection::NetClient;

use crate::hues;

pub trait NetClientExt {
    fn send_system_message(&self, message: impl Into<String>);

    fn send_system_message_hue(&self, message: impl Into<String>, hue: u16);

    fn send_system_message_font(&self, message: impl Into<String>, font: u16, hue: u16);
}

impl NetClientExt for NetClient {
    fn send_system_message(&self, message: impl Into<String>) {
        self.send_system_message_font(message, 3, hues::GREY);
    }

    fn send_system_message_hue(&self, message: impl Into<String>, hue: u16) {
        self.send_system_message_font(message, 3, hue);
    }

    fn send_system_message_font(&self, message: impl Into<String>, font: u16, hue: u16) {
        self.send_packet(UnicodeTextMessage {
            kind: MessageKind::Regular,
            text: message.into(),
            hue,
            font,
            language: FixedString::from_str("ENU"),
            name: FixedString::from_str("System"),
            ..Default::default()
        });
    }
}
