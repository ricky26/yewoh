use derive_more::{Display, Error};
use crate::parser::{SourcePosition, DisplayAddress};

#[derive(Clone, Debug, Error, Display)]
pub enum ParseError<P: SourcePosition> {
    #[display("{}: missing end quote", DisplayAddress(_0))]
    MissingEndQuote(P),
    #[display("{}: invalid escape '\\{_1}'", DisplayAddress(_0))]
    InvalidEscape(P, char),
    #[display("{}: unexpected end of file in escape sequence", DisplayAddress(_0))]
    EofDuringEscape(P),
}

impl<P: SourcePosition> ParseError<P> {
    pub fn map_position<P2: SourcePosition>(self, mut f: impl FnMut(P) -> P2) -> ParseError<P2> {
        match self {
            ParseError::MissingEndQuote(p) => ParseError::MissingEndQuote(f(p)),
            ParseError::InvalidEscape(p, c) => ParseError::InvalidEscape(f(p), c),
            ParseError::EofDuringEscape(p) => ParseError::EofDuringEscape(f(p)),
        }
    }
}

enum StringFragment<'a> {
    Literal(&'a str),
    EscapedChar(char),
    EndOfString,
    PartialEscape,
    InvalidEscape(char),
}

struct StringIterator<'a> {
    string: &'a str,
}

impl<'a> StringIterator<'a> {
    pub fn try_new(src: &'a str) -> Option<StringIterator<'a>> {
       if !src.starts_with('"') {
           None
       } else {
           Some(StringIterator {
               string: &src[1..],
           })
       }
    }

    pub fn as_str(&self) -> &'a str {
        self.string
    }
}

impl<'a> Iterator for StringIterator<'a> {
    type Item = StringFragment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.string.is_empty() {
            return None;
        }

        let mut chars = self.string.chars();
        match chars.next().unwrap() {
            '"' => {
                self.string = chars.as_str();
                return Some(StringFragment::EndOfString);
            }
            '\\' => {
                let Some(c) = chars.next() else {
                    self.string = "";
                    return Some(StringFragment::PartialEscape);
                };

                let c = match c {
                    'r' => '\r',
                    'n' => '\n',
                    't' => '\t',
                    '\"' => '\"',
                    '\'' => '\'',
                    c => return Some(StringFragment::InvalidEscape(c)),
                };

                self.string = chars.as_str();
                return Some(StringFragment::EscapedChar(c));
            }
            _ => {}
        }

        match self.string.find(&['"', '\\']) {
            None => {
                let res = StringFragment::Literal(self.string);
                self.string = "";
                Some(res)
            }
            Some(offset) => {
                let (left, right) = self.string.split_at(offset);
                self.string = right;
                Some(StringFragment::Literal(left))
            }
        }
    }
}

pub fn recognize_string(src: &str) -> Result<Option<(&str, &str)>, ParseError<&str>> {
    let mut it = match StringIterator::try_new(src) {
        Some(v) => v,
        None => return Ok(None),
    };
    while let Some(fragment) = it.next() {
        match fragment {
            StringFragment::EndOfString => {
                let rest = it.as_str();
                let used = &src[..(src.len() - rest.len())];
                return Ok(Some((rest, used)));
            }
            StringFragment::InvalidEscape(c) =>
                return Err(ParseError::InvalidEscape(it.as_str(), c)),
            StringFragment::PartialEscape =>
                return Err(ParseError::EofDuringEscape(src)),
            _ => {}
        }
    }

    Err(ParseError::MissingEndQuote(src))
}

pub fn parse_string(src: &str) -> Result<Option<(&str, String)>, ParseError<&str>> {
    let mut result = String::new();
    let mut it = match StringIterator::try_new(src) {
        Some(v) => v,
        None => return Ok(None),
    };
    while let Some(fragment) = it.next() {
        match fragment {
            StringFragment::Literal(s) => result.push_str(s),
            StringFragment::EscapedChar(c) => result.push(c),
            StringFragment::EndOfString => {
                let rest = it.as_str();
                return Ok(Some((rest, result)));
            }
            StringFragment::InvalidEscape(c) =>
                return Err(ParseError::InvalidEscape(it.as_str(), c)),
            StringFragment::PartialEscape =>
                return Err(ParseError::EofDuringEscape(src)),
        }
    }

    Err(ParseError::MissingEndQuote(src))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let (rest, s) = recognize_string("\"test\\\"1234\\r\\n\\t\" padding").unwrap().unwrap();
        assert_eq!(rest, " padding");
        assert_eq!(s, "\"test\\\"1234\\r\\n\\t\"");
    }
}
