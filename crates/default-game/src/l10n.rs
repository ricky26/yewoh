use std::borrow::Cow;
use std::fmt::{Display, Formatter};

use anyhow::{anyhow, Context};
use bevy::prelude::*;
use bevy_fabricator::parser::FormatterFn;
use bevy_fabricator::traits::{Convert, ReflectConvert};

pub const EMPTY_TEXT_1: u32 = 1042971;
pub const EMPTY_TEXT_2: u32 = 1070722;
pub const EMPTY_TEXT_3: u32 = 1114057;
pub const EMPTY_TEXT_4: u32 = 1114778;
pub const EMPTY_TEXT_5: u32 = 1114779;
pub const EMPTY_TEXT_6: u32 = 1150541;
pub const EMPTY_TEXT_7: u32 = 1151408;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Reflect)]
#[reflect(Convert)]
pub struct LocalisedString<'a> {
    pub text_id: u32,
    pub arguments: Cow<'a, str>,
}

impl Default for LocalisedString<'_> {
    fn default() -> Self {
        LocalisedString {
            text_id: EMPTY_TEXT_1,
            arguments: Cow::Borrowed(""),
        }
    }
}

impl<'a> LocalisedString<'a> {
    pub fn from_str(text: impl Into<Cow<'a, str>>) -> LocalisedString<'a> {
        Self {
            text_id: EMPTY_TEXT_1,
            arguments: text.into(),
        }
    }

    pub fn from_id(text_id: u32) -> LocalisedString<'a> {
        LocalisedString {
            text_id,
            arguments: Cow::Borrowed(""),
        }
    }

    pub fn as_argument(&self) -> impl Display + '_ {
        FormatterFn(|f: &mut Formatter| {
            if self.arguments.is_empty() {
                write!(f, "#{}", self.text_id)
            } else if self.text_id == EMPTY_TEXT_1 {
                f.write_str(&self.arguments)
            } else {
                warn!("tried to format l10n string");
                write!(f, "#{}<{}>", self.text_id, self.arguments)
            }
        })
    }
}

impl<'a> Convert for LocalisedString<'a> {
    fn convert(from: Box<dyn PartialReflect>) -> anyhow::Result<Box<dyn PartialReflect>> {
        let from = match from.try_downcast::<LocalisedString<'static>>() {
            Ok(value) => return Ok(value),
            Err(value) => value,
        };

        if let Some(s) = String::from_reflect(from.as_ref()) {
            Ok(Box::new(LocalisedString::from_str(s)))
        } else {
            let s = from.reflect_ref().as_struct()
                .with_context(|| format!("value {from:?}"))?;
            let mut text_id = EMPTY_TEXT_1;
            let mut arguments = Cow::Borrowed("");

            if let Some(id) = s.field("text_id") {
                text_id = u32::from_reflect(id)
                    .ok_or_else(|| anyhow!("expected text_id to be of type u32 (got {id:?})"))?;
            }

            if let Some(args) = s.field("arguments") {
                let args = <&str>::from_reflect(args)
                    .ok_or_else(|| anyhow!("expected arguments to be a string (got {args:?})"))?;
                arguments = Cow::Owned(args.to_owned());
            }

            Ok(Box::new(LocalisedString { text_id, arguments }))
        }
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<LocalisedString<'static>>()
        .register_type_data::<Cow<'static, str>, ReflectFromReflect>();
}
