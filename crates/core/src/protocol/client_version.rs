use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use anyhow::anyhow;
use bitflags::bitflags;

pub const VERSION_HIGH_SEAS: ClientVersion = ClientVersion::new(7, 0, 9, 0);
pub const VERSION_GRID_INVENTORY: ClientVersion = ClientVersion::new(6, 0, 1, 7);

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct ClientVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub build: u8,
}

impl ClientVersion {
    pub const fn new(major: u8, minor: u8, patch: u8, build: u8) -> ClientVersion {
        Self {
            major,
            minor,
            patch,
            build,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.major != 0 || self.minor != 0 || self.patch != 0 || self.build != 0
    }
}

impl fmt::Display for ClientVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}.{}", self.major, self.minor, self.patch, self.build)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct ExtendedClientVersion {
    pub client_version: ClientVersion,
    pub suffix: String,
}

impl ExtendedClientVersion {
    pub fn new(major: u8, minor: u8, patch: u8, build: u8, suffix: impl Into<String>) -> Self {
        Self {
            client_version: ClientVersion::new(major, minor, patch, build),
            suffix: suffix.into(),
        }
    }
}

impl Deref for ExtendedClientVersion {
    type Target = ClientVersion;

    fn deref(&self) -> &Self::Target { &self.client_version }
}

impl FromStr for ExtendedClientVersion {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (major_str, rest) = s.split_once('.')
            .ok_or_else(|| anyhow!("Missing major . in version"))?;
        let major = u8::from_str(major_str)?;
        let (minor_str, rest) = rest.split_once('.')
            .ok_or_else(|| anyhow!("Missing minor . in version"))?;
        let minor = u8::from_str(minor_str)?;

        let (patch, build, suffix) = if let Some((patch_str, rest)) = rest.split_once('.') {
            let patch = u8::from_str(patch_str)?;
            let (build_str, rest) = rest.split_once(|c: char| !c.is_digit(10))
                .unwrap_or_else(|| (rest, ""));
            let build = u8::from_str(build_str)?;
            (patch, build, rest)
        } else {
            let (patch_str, rest) = rest.split_once(|c: char| !c.is_digit(10))
                .ok_or_else(|| anyhow!("Missing patch number"))?;
            let patch = u8::from_str(patch_str)?;
            let first_char = rest.chars().next().unwrap_or('\0');
            if rest.len() == 1 && first_char >= 'a' && first_char <= 'z' {
                let build = (first_char as u8 - b'a') as u8;
                (patch, build, "")
            } else {
                (patch, 0, rest)
            }
        };

        Ok(ExtendedClientVersion::new(major, minor, patch, build, suffix))
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, Default)]
    pub struct ClientFlags: u32 {
        const RE = 0x1;
        const TD = 0x2;
        const LBR = 0x4;
        const AOS = 0x8;
        const SE = 0x10;
        const SA = 0x20;
        const UO3D = 0x40;
        const THREE_D = 0x100;
    }
}
