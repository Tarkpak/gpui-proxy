use crate::proxy::Node;
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use url::Url;

const MAX_BODY: u64 = 2 * 1024 * 1024;
pub const REFRESH_INTERVAL: u64 = 24 * 60 * 60;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscription {
    pub url: String,
    pub links: Vec<String>,
    pub updated: u64,
    pub skipped: usize,
}

impl Subscription {
    pub fn name(&self) -> String {
        Url::parse(&self.url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .unwrap_or_else(|| "订阅链接".into())
    }
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn validate(text: &str) -> Result<Url> {
    let mut url = Url::parse(text.trim()).context("请输入有效的订阅链接")?;
    ensure!(
        matches!(url.scheme(), "http" | "https") && url.host().is_some(),
        "订阅链接必须以 http:// 或 https:// 开头"
    );
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "订阅链接不支持 URL 用户名和密码"
    );
    url.set_fragment(None);
    Ok(url)
}

pub fn client(proxy: Option<SocketAddr>) -> Result<reqwest::blocking::Client> {
    let mut builder = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(6))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let downgrade = attempt.previous().last().is_some_and(|previous| {
                previous.scheme() == "https" && attempt.url().scheme() != "https"
            });
            if downgrade
                || attempt.previous().len() >= 5
                || validate(attempt.url().as_str()).is_err()
            {
                attempt.error("invalid redirect")
            } else {
                attempt.follow()
            }
        }))
        .user_agent("Xray Connect");
    if let Some(address) = proxy {
        builder = builder.proxy(reqwest::Proxy::all(format!("socks5h://{address}"))?);
    }
    builder
        .build()
        .map_err(|_| anyhow::anyhow!("无法初始化订阅请求"))
}

pub fn fetch(client: &reqwest::blocking::Client, url: Url) -> Result<Subscription> {
    let response = client.get(url.clone()).send().map_err(|error| {
        if error.is_timeout() {
            anyhow::anyhow!("订阅请求超时")
        } else {
            anyhow::anyhow!("无法获取订阅，请检查链接和网络")
        }
    })?;
    ensure!(
        response.status().is_success(),
        "订阅服务器返回 HTTP {}",
        response.status().as_u16()
    );
    ensure!(
        response.content_length().unwrap_or(0) <= MAX_BODY,
        "订阅内容超过 2 MiB"
    );
    let mut bytes = Vec::new();
    response
        .take(MAX_BODY + 1)
        .read_to_end(&mut bytes)
        .context("读取订阅内容失败")?;
    ensure!(bytes.len() as u64 <= MAX_BODY, "订阅内容超过 2 MiB");
    let (links, skipped) = parse(std::str::from_utf8(&bytes).context("订阅内容不是 UTF-8 文本")?)?;
    Ok(Subscription {
        url: url.into(),
        links,
        skipped,
        updated: now(),
    })
}

fn parse(body: &str) -> Result<(Vec<String>, usize)> {
    let text = body.trim().trim_start_matches('\u{feff}').trim();
    let decoded = if !text.contains("://") && !text.contains(':') {
        let compact: String = text
            .chars()
            .filter(|ch| !ch.is_ascii_whitespace())
            .collect();
        [
            general_purpose::STANDARD,
            general_purpose::STANDARD_NO_PAD,
            general_purpose::URL_SAFE,
            general_purpose::URL_SAFE_NO_PAD,
        ]
        .iter()
        .find_map(|engine| engine.decode(&compact).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok())
    } else {
        None
    };
    let text = decoded.as_deref().unwrap_or(text);
    let mut links = Vec::new();
    let mut known = std::collections::HashSet::new();
    let mut skipped = 0;
    let mut push = |link: Option<String>| {
        if let Some(link) = link.filter(|link| Node::parse(link).is_ok()) {
            let identity = link
                .split_once('#')
                .map_or(link.as_str(), |(prefix, _)| prefix);
            if known.insert(identity.to_owned()) {
                links.push(link);
            }
        } else {
            skipped += 1;
        }
    };
    if let Ok(clash) = serde_saphyr::from_str::<Clash>(text) {
        for proxy in clash.proxies {
            push(match proxy {
                ClashEntry::Proxy(proxy) => proxy.link(),
                ClashEntry::Other(_) => None,
            });
        }
    } else {
        for line in text
            .lines()
            .map(str::trim)
            .filter(|line| line.contains("://"))
        {
            push(Some(line.to_owned()));
        }
    }
    if links.is_empty() {
        bail!("订阅没有可用的 VLESS TCP 节点");
    }
    Ok((links, skipped))
}

#[derive(Deserialize)]
struct Clash {
    proxies: Vec<ClashEntry>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ClashEntry {
    Proxy(Box<ClashProxy>),
    Other(serde::de::IgnoredAny),
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ClashProxy {
    #[serde(rename = "type")]
    kind: String,
    server: String,
    port: u16,
    uuid: String,
    name: Option<String>,
    network: Option<String>,
    tls: Option<bool>,
    servername: Option<String>,
    flow: Option<String>,
    client_fingerprint: Option<String>,
    alpn: Option<Vec<String>>,
    reality_opts: Option<Reality>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Reality {
    public_key: String,
    short_id: Option<String>,
}

impl ClashProxy {
    fn link(self) -> Option<String> {
        if self.kind != "vless" || !matches!(self.network.as_deref(), None | Some("tcp" | "raw")) {
            return None;
        }
        let server = match self.server.parse::<std::net::Ipv6Addr>() {
            Ok(ip) => format!("[{ip}]"),
            Err(_) => self.server,
        };
        let mut url = Url::parse(&format!("vless://{}@{server}:{}", self.uuid, self.port)).ok()?;
        let mut query = url.query_pairs_mut();
        let security = if self.reality_opts.is_some() {
            "reality"
        } else if self.tls == Some(true) {
            "tls"
        } else {
            "none"
        };
        query.append_pair("security", security);
        for (key, value) in [
            ("sni", self.servername),
            ("flow", self.flow),
            ("fp", self.client_fingerprint),
            ("alpn", self.alpn.map(|values| values.join(","))),
        ] {
            if let Some(value) = value {
                query.append_pair(key, &value);
            }
        }
        if let Some(reality) = self.reality_opts {
            query.append_pair("pbk", &reality.public_key);
            if let Some(id) = reality.short_id {
                query.append_pair("sid", &id);
            }
        }
        drop(query);
        url.set_fragment(self.name.as_deref());
        Some(url.into())
    }
}

pub fn storage_path() -> Result<PathBuf> {
    Ok(dirs::data_local_dir()
        .context("无法找到本地数据目录")?
        .join("Xray Connect")
        .join("subscriptions.json"))
}

pub fn load(path: &Path) -> Result<Vec<Subscription>> {
    match fs::read(path) {
        Ok(data) => serde_json::from_slice(&data).context("无法读取已保存的订阅"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error).context("无法打开订阅文件"),
    }
}

fn save(path: &Path, expected: &[Subscription], subscriptions: &[Subscription]) -> Result<()> {
    let parent = path.parent().context("缺少订阅数据目录")?;
    fs::create_dir_all(parent)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path.with_extension("lock"))?;
    lock.lock()?;
    ensure!(
        load(path)? == expected,
        "订阅文件已由另一个窗口修改，请重新打开客户端"
    );
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(file.as_file_mut(), subscriptions)?;
    file.flush()?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}

type RefreshResult = Result<(Vec<Subscription>, String, bool)>;

pub struct Manager {
    pub records: Vec<Subscription>,
    pub pending: Option<Url>,
    pub feedback: String,
    pub failed: bool,
    path: Option<PathBuf>,
    job: Option<std::sync::mpsc::Receiver<RefreshResult>>,
    next_auto: std::time::Instant,
}

impl Manager {
    pub fn new(path: Result<PathBuf>) -> Self {
        let mut manager = Self {
            records: Vec::new(),
            pending: None,
            feedback: String::new(),
            failed: false,
            path: None,
            job: None,
            next_auto: std::time::Instant::now(),
        };
        match path.and_then(|path| load(&path).map(|records| (path, records))) {
            Ok((path, records)) => {
                manager.path = Some(path);
                manager.records = records;
            }
            Err(error) => manager.error(error.to_string()),
        }
        manager
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    pub fn writable(&self) -> bool {
        self.path.is_some()
    }

    pub fn error(&mut self, message: String) {
        self.failed = true;
        self.feedback = message;
    }

    pub fn start(&mut self, urls: Vec<String>, proxy: Option<SocketAddr>) {
        if self.busy() || urls.is_empty() {
            return;
        }
        let Some(path) = self.path.clone() else {
            return;
        };
        let records = self.records.clone();
        let (send, receive) = std::sync::mpsc::channel();
        self.job = Some(receive);
        self.failed = false;
        self.feedback = "正在更新订阅…".into();
        std::thread::spawn(move || {
            let _ = send.send(refresh(&path, records, urls, proxy));
        });
    }

    pub fn poll(&mut self, proxy: Option<SocketAddr>, auto_refresh: bool) -> (bool, bool) {
        if let Some(job) = &self.job {
            let result = match job.try_recv() {
                Ok(result) => result,
                Err(std::sync::mpsc::TryRecvError::Empty) => return (false, false),
                Err(_) => Err(anyhow::anyhow!("订阅更新意外中断")),
            };
            self.job = None;
            return match result {
                Ok((records, feedback, failed)) => {
                    if self
                        .pending
                        .as_ref()
                        .is_some_and(|url| records.iter().any(|record| record.url == url.as_str()))
                    {
                        self.pending = None;
                    }
                    self.records = records;
                    self.feedback = feedback;
                    self.failed = failed;
                    (true, true)
                }
                Err(error) => {
                    self.error(error.to_string());
                    (true, false)
                }
            };
        }
        if auto_refresh && std::time::Instant::now() >= self.next_auto {
            self.next_auto = std::time::Instant::now() + Duration::from_secs(60 * 60);
            let urls: Vec<_> = self
                .records
                .iter()
                .filter(|item| now().saturating_sub(item.updated) >= REFRESH_INTERVAL)
                .map(|item| item.url.clone())
                .collect();
            self.start(urls, proxy);
            return (self.busy(), false);
        }
        (false, false)
    }

    pub fn remove(&mut self, index: usize) -> bool {
        if self.busy() || index >= self.records.len() {
            return false;
        }
        let Some(path) = &self.path else { return false };
        let mut records = self.records.clone();
        records.remove(index);
        if let Err(error) = save(path, &self.records, &records) {
            self.error(format!("保存订阅失败。{error}"));
            return false;
        }
        self.records = records;
        self.feedback = "已移除订阅".into();
        self.failed = false;
        true
    }
}

fn refresh(
    path: &Path,
    mut records: Vec<Subscription>,
    urls: Vec<String>,
    proxy: Option<SocketAddr>,
) -> RefreshResult {
    let client = client(proxy)?;
    let expected = records.clone();
    let mut updated = 0;
    let mut failures = Vec::new();
    for url in urls {
        match validate(&url).and_then(|url| fetch(&client, url)) {
            Ok(record) => {
                if let Some(item) = records.iter_mut().find(|item| item.url == record.url) {
                    *item = record;
                } else {
                    records.push(record);
                }
                updated += 1;
            }
            Err(error) => failures.push(error.to_string()),
        }
    }
    if updated > 0 {
        save(path, &expected, &records)
            .map_err(|error| anyhow::anyhow!("保存订阅失败。{error}"))?;
    }
    let feedback = match failures.first() {
        None => format!("已更新 {updated} 条订阅"),
        Some(error) if updated == 0 && failures.len() == 1 => error.clone(),
        Some(error) => format!("已更新 {updated} 条，失败 {} 条。{error}", failures.len()),
    };
    Ok((records, feedback, !failures.is_empty()))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::net::TcpListener;

    const LINK: &str = "vless://6cece960-04ec-4655-800e-bb0a28c290d1@127.0.0.1:443?security=tls";

    pub fn serve(responses: Vec<(u16, String)>) -> String {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for (status, body) in responses {
                let (mut connection, _) = listener.accept().unwrap();
                connection
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = std::io::BufReader::new(&connection);
                use std::io::BufRead;
                let mut line = String::new();
                while request.read_line(&mut line).unwrap() != 0 && line != "\r\n" {
                    line.clear();
                }
                write!(connection, "HTTP/1.1 {status} Response\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        format!("http://{address}/subscriptions")
    }

    #[test]
    fn auto_refresh_can_be_disabled_without_disabling_manual_refresh() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("feeds.json");
        let url = serve(vec![(200, LINK.into())]);
        save(
            &path,
            &[],
            &[Subscription {
                url: url.clone(),
                links: vec![LINK.into()],
                updated: 0,
                skipped: 0,
            }],
        )
        .unwrap();
        let mut manager = Manager::new(Ok(path));
        assert_eq!(manager.poll(None, false), (false, false));
        assert!(!manager.busy());
        manager.start(vec![url], None);
        assert!(manager.busy());
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while manager.busy() {
            manager.poll(None, false);
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!manager.failed);
    }

    #[test]
    fn parses_text_base64_and_clash_without_importing_unsupported_nodes() {
        let text = format!("{LINK}#one\n{LINK}#duplicate\nvmess://ignored\n");
        assert_eq!(parse(&text).unwrap(), (vec![format!("{LINK}#one")], 1));
        assert_eq!(
            parse(&general_purpose::URL_SAFE_NO_PAD.encode(&text)).unwrap(),
            parse(&text).unwrap()
        );
        let yaml = "proxies:\n  - name: Server\n    type: vless\n    server: 127.0.0.1\n    port: 443\n    uuid: 6cece960-04ec-4655-800e-bb0a28c290d1\n    tls: true\n  - name: Unsupported\n    type: vmess\n";
        assert_eq!(parse(yaml).unwrap(), (vec![format!("{LINK}#Server")], 1));
        assert!(parse("vmess://ignored").is_err());
    }

    #[test]
    fn preserves_previous_snapshot_on_failed_refresh_and_applies_successful_refresh() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("feeds.json");
        let replacement = LINK.replace(":443", ":8443");
        let url = serve(vec![
            (200, LINK.into()),
            (503, String::new()),
            (200, replacement.clone()),
        ]);
        let (records, _, failed) = refresh(&path, vec![], vec![url.clone()], None).unwrap();
        assert!(!failed);
        assert_eq!(records[0].links, [LINK]);
        let (records, _, failed) = refresh(&path, records, vec![url.clone()], None).unwrap();
        assert!(failed);
        assert_eq!(records[0].links, [LINK]);
        assert_eq!(load(&path).unwrap()[0].links, [LINK]);
        let (_, _, failed) = refresh(&path, records, vec![url], None).unwrap();
        assert!(!failed);
        assert_eq!(load(&path).unwrap()[0].links, [replacement]);
        let mut manager = Manager::new(Ok(path.clone()));
        assert!(manager.remove(0));
        assert!(load(&path).unwrap().is_empty());
    }
}
