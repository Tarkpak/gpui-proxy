use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::proxy::Mode;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub default_mode: Mode,
    pub xray_path: Option<PathBuf>,
    pub sing_box_path: Option<PathBuf>,
    pub auto_refresh: bool,
    pub access_logs: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_mode: Mode::Local,
            xray_path: None,
            sing_box_path: None,
            auto_refresh: true,
            access_logs: true,
        }
    }
}

#[derive(Clone, Copy)]
pub enum Core {
    Xray,
    SingBox,
}

impl Core {
    pub fn label(self) -> &'static str {
        match self {
            Self::Xray => "Xray",
            Self::SingBox => "sing-box",
        }
    }

    pub fn path(self, settings: &Settings) -> &Option<PathBuf> {
        match self {
            Self::Xray => &settings.xray_path,
            Self::SingBox => &settings.sing_box_path,
        }
    }

    pub fn set_path(self, settings: &mut Settings, path: Option<PathBuf>) {
        match self {
            Self::Xray => settings.xray_path = path,
            Self::SingBox => settings.sing_box_path = path,
        }
    }
}

pub fn validate_executable(path: PathBuf) -> Result<PathBuf> {
    ensure!(
        path.is_absolute() && path.is_file(),
        "请选择已存在的核心可执行文件"
    );
    #[cfg(windows)]
    ensure!(
        path.extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe")),
        "Windows 核心文件必须是 .exe"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        ensure!(
            fs::metadata(&path)?.permissions().mode() & 0o111 != 0,
            "文件缺少执行权限"
        );
    }
    Ok(path)
}

pub fn path() -> Result<PathBuf> {
    Ok(dirs::data_local_dir()
        .context("无法找到本地数据目录")?
        .join("Xray Connect")
        .join("settings.json"))
}

pub fn load(path: &Path) -> Result<Settings> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).context("无法解析客户端设置"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(error) => Err(error).context("无法读取客户端设置"),
    }
}

pub fn save(path: &Path, expected: &Settings, value: &Settings) -> Result<()> {
    let parent = path.parent().context("无法找到设置文件目录")?;
    fs::create_dir_all(parent)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path.with_extension("lock"))?;
    lock.lock()?;
    ensure!(
        load(path)? == *expected,
        "设置已在另一个窗口修改，请重新打开客户端"
    );
    atomic_write(path, &serde_json::to_vec_pretty(value)?)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("缺少文件目录")?;
    fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.flush()?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}

#[cfg(windows)]
mod startup {
    use super::*;
    use windows_registry::CURRENT_USER;
    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "Xray Connect";

    fn missing(code: i32) -> bool {
        matches!(code as u32, 0x80070002 | 0x80070003)
    }

    pub fn enabled() -> Result<bool> {
        is_registered(RUN)
    }

    fn is_registered(run: &str) -> Result<bool> {
        match CURRENT_USER.open(run).and_then(|key| key.get_string(NAME)) {
            Ok(_) => Ok(true),
            Err(error) if missing(error.code().0) => Ok(false),
            Err(error) => Err(error).context("无法读取登录启动项"),
        }
    }

    pub fn set(enabled: bool) -> Result<()> {
        set_registration(RUN, enabled)
    }

    fn set_registration(run: &str, enabled: bool) -> Result<()> {
        if enabled {
            let executable = std::env::current_exe()?;
            let executable = executable.to_str().context("应用路径无法写入启动项")?;
            ensure!(!executable.contains('"'), "应用路径不能包含引号");
            CURRENT_USER
                .create(run)?
                .set_string(NAME, format!("\"{executable}\""))?;
        } else {
            match CURRENT_USER
                .options()
                .write()
                .open(run)
                .and_then(|key| key.remove_value(NAME))
            {
                Ok(()) => {}
                Err(error) if missing(error.code().0) => {}
                Err(error) => return Err(error).context("无法移除登录启动项"),
            }
        }
        Ok(())
    }

    #[cfg(test)]
    #[test]
    fn windows_login_registration_round_trip_in_temporary_key() {
        let directory = tempfile::tempdir().unwrap();
        let suffix = directory.path().file_name().unwrap().to_string_lossy();
        let path = format!(r"Software\Xray Connect verification {suffix}");
        struct RemoveKey(String);
        impl Drop for RemoveKey {
            fn drop(&mut self) {
                let _ = CURRENT_USER.remove_tree(&self.0);
            }
        }
        let _cleanup = RemoveKey(path.clone());
        assert!(!is_registered(&path).unwrap());
        set_registration(&path, true).unwrap();
        assert!(is_registered(&path).unwrap());
        let saved = CURRENT_USER.open(&path).unwrap().get_string(NAME).unwrap();
        assert_eq!(
            saved,
            format!("\"{}\"", std::env::current_exe().unwrap().display())
        );
        set_registration(&path, false).unwrap();
        assert!(!is_registered(&path).unwrap());
    }
}

#[cfg(target_os = "macos")]
mod startup {
    use super::*;

    fn agent() -> Result<PathBuf> {
        // The client can be launched with sudo for TUN. The login item belongs
        // to the desktop user, so register it from a normal launch.
        ensure!(
            unsafe { libc::geteuid() } != 0,
            "请以普通用户身份打开客户端来设置登录启动"
        );
        Ok(dirs::home_dir()
            .context("无法找到用户目录")?
            .join("Library/LaunchAgents/io.xray-connect.client.plist"))
    }

    pub fn enabled() -> Result<bool> {
        Ok(agent()?.try_exists()?)
    }

    pub fn set(enabled: bool) -> Result<()> {
        let agent = agent()?;
        if enabled {
            let path = std::env::current_exe()?;
            let path = path
                .to_str()
                .context("应用路径无法写入启动项")?
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            let plist = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>io.xray-connect.client</string>
<key>ProgramArguments</key><array><string>{path}</string></array>
<key>LimitLoadToSessionType</key><string>Aqua</string>
<key>RunAtLoad</key><true/>
</dict></plist>"#
            );
            atomic_write(&agent, plist.as_bytes())
        } else {
            match fs::remove_file(agent) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error).context("无法移除登录启动项"),
            }
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod startup {
    use super::*;
    pub fn enabled() -> Result<bool> {
        anyhow::bail!("当前平台不支持登录启动")
    }
    pub fn set(_: bool) -> Result<()> {
        anyhow::bail!("当前平台不支持登录启动")
    }
}

pub use startup::{enabled as startup_enabled, set as set_startup};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_do_not_overwrite_concurrent_edits() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let defaults = load(&path).unwrap();
        let updated = Settings {
            default_mode: Mode::Tun,
            xray_path: Some(directory.path().join("xray.exe")),
            auto_refresh: false,
            access_logs: false,
            ..defaults.clone()
        };
        save(&path, &defaults, &updated).unwrap();
        assert_eq!(load(&path).unwrap(), updated);
        assert!(save(&path, &defaults, &defaults).is_err());
        assert_eq!(load(&path).unwrap(), updated);
        save(&path, &updated, &defaults).unwrap();
        assert_eq!(load(&path).unwrap(), defaults);
    }
}
