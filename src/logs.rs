use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Client,
    Xray,
    SingBox,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Self::Client => "客户端",
            Self::Xray => "Xray",
            Self::SingBox => "sing-box",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Traffic,
    Warning,
    Error,
}

impl Level {
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "信息",
            Self::Traffic => "访问",
            Self::Warning => "警告",
            Self::Error => "错误",
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Filter {
    #[default]
    All,
    Traffic,
    Core,
    Issues,
}

impl Filter {
    pub const ALL: [Self; 4] = [Self::All, Self::Traffic, Self::Core, Self::Issues];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "全部",
            Self::Traffic => "访问",
            Self::Core => "核心",
            Self::Issues => "警告与错误",
        }
    }

    pub fn accepts(self, entry: &Entry) -> bool {
        match self {
            Self::All => true,
            Self::Traffic => entry.level == Level::Traffic,
            Self::Core => entry.source != Source::Client && entry.level != Level::Traffic,
            Self::Issues => matches!(entry.level, Level::Warning | Level::Error),
        }
    }
}

pub struct CoreLine {
    pub source: Source,
    pub text: String,
    pub stderr: bool,
}

impl CoreLine {
    pub fn level(&self) -> Level {
        let line = self.text.as_str();
        if line.contains("[Error]")
            || line.contains("ERROR[")
            || line.contains("FATAL[")
            || line.starts_with("ERROR ")
            || line.starts_with("FATAL ")
        {
            Level::Error
        } else if line.contains("[Warning]") || line.contains("WARN[") || line.starts_with("WARN ")
        {
            Level::Warning
        } else if (self.source == Source::Xray
            && (line.contains(" accepted ") || line.contains(" rejected ")))
            || (self.source == Source::SingBox
                && (line.contains("connection to ")
                    || line.contains("connection from ")
                    || line.contains("dns: exchanged")))
        {
            Level::Traffic
        } else if self.stderr && self.source == Source::Xray {
            Level::Warning
        } else {
            Level::Info
        }
    }
}

pub struct Entry {
    pub elapsed: Duration,
    pub source: Source,
    pub level: Level,
    pub title: &'static str,
    pub detail: String,
}

impl Entry {
    pub fn time(&self) -> String {
        let seconds = self.elapsed.as_secs();
        format!(
            "{:02}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    }

    pub fn message(&self) -> &str {
        if self.detail.is_empty() {
            self.title
        } else {
            &self.detail
        }
    }
}
