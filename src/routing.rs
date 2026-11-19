use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, io::Write, path::Path, str::FromStr};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    #[default]
    Proxy,
    Direct,
    Block,
}

impl Action {
    pub const ALL: [Self; 3] = [Self::Proxy, Self::Direct, Self::Block];

    pub fn label(self) -> &'static str {
        match self {
            Self::Proxy => "代理",
            Self::Direct => "直连",
            Self::Block => "拦截",
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Self::Proxy => "xray",
            Self::Direct => "direct",
            Self::Block => "block",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    Domain(String),
    Exact(String),
    Network(ipnet::IpNet),
}

impl Target {
    fn parse(text: &str) -> Result<Self> {
        if let Ok(network) = ipnet::IpNet::from_str(text) {
            return Ok(Self::Network(network.trunc()));
        }
        if let Ok(address) = std::net::IpAddr::from_str(text) {
            return Ok(Self::Network(address.into()));
        }
        let (kind, domain) = text.split_once(':').unwrap_or(("domain", text));
        ensure!(
            matches!(kind, "domain" | "full"),
            "匹配项必须是域名、IP 或 CIDR"
        );
        let domain = domain.trim_end_matches('.');
        let url::Host::Domain(domain) = url::Host::parse(domain).context("域名无效")? else {
            anyhow::bail!("请直接填写 IP 或 CIDR");
        };
        ensure!(
            domain.split('.').all(|label| !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                && !label.starts_with('-')
                && !label.ends_with('-'))
                && domain.len() <= 253,
            "域名无效"
        );
        Ok(if kind == "full" {
            Self::Exact(domain)
        } else {
            Self::Domain(domain)
        })
    }

    pub fn label(&self) -> String {
        match self {
            Self::Domain(name) => name.clone(),
            Self::Exact(name) => format!("full:{name}"),
            Self::Network(network) => network.to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub target: Target,
    pub action: Action,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rules {
    pub entries: Vec<Rule>,
    pub fallback: Action,
}

impl Rules {
    pub fn import(text: &str) -> Result<Vec<Rule>> {
        let mut rules = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let result = (|| -> Result<Rule> {
                let mut parts = line.split_whitespace();
                let action = match parts.next() {
                    Some("proxy" | "代理") => Action::Proxy,
                    Some("direct" | "直连") => Action::Direct,
                    Some("block" | "拦截") => Action::Block,
                    _ => anyhow::bail!("动作必须为 proxy、direct 或 block"),
                };
                let target = Target::parse(parts.next().context("缺少域名、IP 或 CIDR")?)?;
                ensure!(parts.next().is_none(), "一行只能填写一个匹配项");
                Ok(Rule { target, action })
            })();
            rules.push(result.with_context(|| format!("第 {} 行", index + 1))?);
        }
        ensure!(!rules.is_empty(), "剪贴板中没有可导入的规则");
        Ok(rules)
    }

    pub fn xray_rules(&self) -> Vec<Value> {
        let mut rules: Vec<_> = self
            .entries
            .iter()
            .map(|rule| {
                let mut value = json!({"type": "field", "outboundTag": rule.action.tag()});
                match &rule.target {
                    Target::Domain(name) => value["domain"] = json!([format!("domain:{name}")]),
                    Target::Exact(name) => value["domain"] = json!([format!("full:{name}")]),
                    Target::Network(network) => value["ip"] = json!([network.to_string()]),
                }
                value
            })
            .collect();
        if self.fallback != Action::Proxy {
            rules.push(
                json!({"type": "field", "network": "tcp,udp", "outboundTag": self.fallback.tag()}),
            );
        }
        rules
    }

    pub fn tun_rules(&self) -> Vec<Value> {
        let mut rules = vec![json!({"port": 53, "action": "hijack-dns"})];
        if self
            .entries
            .iter()
            .any(|rule| !matches!(rule.target, Target::Network(_)))
        {
            rules.push(json!({"action": "sniff"}));
        }
        rules.extend(self.entries.iter().map(|rule| {
            let mut value = match rule.action {
                Action::Block => json!({"action": "reject"}),
                action => json!({"action": "route", "outbound": action.tag()}),
            };
            match &rule.target {
                Target::Domain(name) => {
                    value["domain"] = json!([name]);
                    value["domain_suffix"] = json!([format!(".{name}")]);
                }
                Target::Exact(name) => value["domain"] = json!([name]),
                Target::Network(network) => value["ip_cidr"] = json!([network.to_string()]),
            }
            value
        }));
        if self.fallback == Action::Block {
            rules.push(json!({"action": "reject"}));
        }
        rules
    }
}

pub fn path() -> Result<std::path::PathBuf> {
    Ok(dirs::data_local_dir()
        .context("无法找到本地数据目录")?
        .join("Xray Connect")
        .join("routing.json"))
}

pub fn load(path: &Path) -> Result<Rules> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).context("无法解析已保存的规则"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Rules::default()),
        Err(error) => Err(error).context("无法读取已保存的规则"),
    }
}

pub fn save(path: &Path, expected: &Rules, rules: &Rules) -> Result<()> {
    let parent = path.parent().context("无法找到规则文件目录")?;
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
        "规则已在另一个窗口修改，请重新打开客户端"
    );
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(file.as_file_mut(), rules)?;
    file.flush()?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}
