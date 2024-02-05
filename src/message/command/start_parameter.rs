use std::{fmt::Display, str::FromStr};

#[derive(Debug, Clone, Copy)]
pub enum StartParameter {
    Blacklist,
    Greeting,
    Regular,
    Help,
}

impl FromStr for StartParameter {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "" => Ok(Self::Regular),
            "blacklist" => Ok(Self::Blacklist),
            "help" => Ok(Self::Help),
            "beep" => Ok(Self::Greeting),
            _ => Err("invalid start parameter".to_string()),
        }
    }
}

impl Display for StartParameter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Regular => write!(f, ""),
            Self::Blacklist => write!(f, "blacklist"),
            Self::Help => write!(f, "help"),
            Self::Greeting => write!(f, "beep"),
        }
    }
}

impl From<StartParameter> for String {
    fn from(value: StartParameter) -> Self {
        value.to_string()
    }
}
