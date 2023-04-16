use std::error::Error;
use std::fmt::{Display, Formatter};
use std::ops::Deref;

#[derive(Debug, Clone)]
pub struct StringTooLong;

impl Display for StringTooLong {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "string too long to store")
    }
}

impl Error for StringTooLong {}

#[derive(Debug, Clone, Eq)]
pub struct FixedString<const I: usize> {
    length: u16,
    contents: [u8; I],
}

impl<const I: usize> FixedString<I> {
    pub const fn len(&self) -> usize {
        self.length as usize
    }

    pub const fn as_byte_array(&self) -> &[u8; I] {
        &self.contents
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.contents[..self.length as usize]
    }

    pub fn as_str(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(self.as_bytes()) }
    }

    pub fn clear(&mut self) {
        self.length = 0;
        self.contents.fill(0);
    }

    pub fn from_str(s: &str) -> FixedString<I> {
        TryFrom::try_from(s).expect("string value is too long")
    }
}

impl<const I: usize> Default for FixedString<I> {
    fn default() -> Self {
        Self {
            length: 0,
            contents: [0; I],
        }
    }
}

impl<const I: usize> Deref for FixedString<I> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl<const I: usize> TryFrom<&str> for FixedString<I> {
    type Error = StringTooLong;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let bytes = value.as_bytes();
        if bytes.len() > I {
            Err(StringTooLong)
        } else {
            let mut result = FixedString::default();
            result.length = bytes.len() as u16;
            result.contents[..bytes.len()].copy_from_slice(bytes);
            Ok(result)
        }
    }
}

impl<const I: usize> PartialEq<str> for FixedString<I> {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl<const I: usize> PartialEq<FixedString<I>> for FixedString<I> {
    fn eq(&self, other: &FixedString<I>) -> bool {
        self.as_str() == other.as_str()
    }
}
