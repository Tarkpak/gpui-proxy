use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    env, fs,
    io::{BufRead, BufReader, Read, Write},
    net::{IpAddr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread,
    time::{Duration, Instant},
};
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Mode {
    Local,
    Tun,
}

pub enum Event {
    Ready(SocketAddr),
    Stopped,
    Failed(String),
}

#[derive(Clone, PartialEq)]
pub struct Node {
    pub label: String,
    host: String,
    port: u16,
    id: uuid::Uuid,
    security: String,
    tls_settings: Value,
    flow: String,
}

impl Node {
    pub fn parse(text: &str) -> Result<Self> {
        let url = Url::parse(text.trim()).context("分享链接无效")?;
        ensure!(url.scheme() == "vless", "目前支持 VLESS 分享链接");
        let host = url.host().context("链接缺少服务器地址")?;
        let host = match host {
            url::Host::Ipv6(ip) => ip.to_string(),
            host => host.to_string(),
        };
        let port = url
            .port()
            .filter(|port| *port != 0)
            .context("链接缺少有效端口")?;
        let id = uuid::Uuid::parse_str(url.username()).context("VLESS UUID 无效")?;
        let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
        let param = |key, default| params.get(key).map_or(default, |value| value.as_ref());
        ensure!(
            matches!(param("type", "tcp"), "tcp" | "raw"),
            "目前支持 TCP 传输"
        );
        ensure!(
            param("headerType", "none") == "none",
            "目前仅支持无伪装的 TCP"
        );
        ensure!(
            param("encryption", "none") == "none",
            "目前仅支持 encryption=none"
        );
        let security = param("security", "none");
        let tls_settings = match security {
            "tls" => {
                let mut settings = json!({"serverName": param("sni", &host)});
                if let Some(alpn) = params.get("alpn") {
                    settings["alpn"] = json!(alpn.split(',').collect::<Vec<_>>());
                }
                if let Some(fingerprint) = params.get("fp") {
                    settings["fingerprint"] = json!(fingerprint);
                }
                settings
            }
            "reality" => {
                let name = param("sni", "");
                let public_key = param("pbk", "");
                ensure!(
                    !name.is_empty() && !public_key.is_empty(),
                    "REALITY 需要 sni 和 pbk"
                );
                json!({
                    "serverName": name,
                    "password": public_key,
                    "shortId": param("sid", ""),
                    "fingerprint": param("fp", "chrome"),
                    "spiderX": param("spx", "")
                })
            }
            "none" => Value::Null,
            _ => bail!("不支持该安全层"),
        };
        let flow = param("flow", "");
        ensure!(
            flow.is_empty() || (security != "none" && flow == "xtls-rprx-vision"),
            "不支持该 flow"
        );
        let security = security.to_owned();
        let flow = flow.to_owned();
        Ok(Self {
            label: format!("{host}:{port}"),
            host,
            port,
            id,
            security,
            tls_settings,
            flow,
        })
    }

    fn config(
        &self,
        endpoint: IpAddr,
        socks: SocketAddr,
        rules: &crate::routing::Rules,
        access_logs: bool,
    ) -> Value {
        let mut stream = json!({"network": "tcp", "security": self.security});
        if self.security != "none" {
            stream[format!("{}Settings", self.security)] = self.tls_settings.clone();
        }
        json!({
            "log": {"access": if access_logs { "" } else { "none" }, "loglevel": "warning"},
            "inbounds": [{
                "listen": socks.ip(), "port": socks.port(),
                "protocol": "socks", "settings": {"udp": true},
                "sniffing": {
                    "enabled": rules.entries.iter().any(|rule| !matches!(rule.target, crate::routing::Target::Network(_))),
                    "destOverride": ["http", "tls", "quic"], "routeOnly": true
                }
            }],
            "outbounds": [{
                "tag": "xray", "protocol": "vless",
                "settings": {"vnext": [{
                    "address": endpoint, "port": self.port,
                    "users": [{"id": self.id.to_string(), "encryption": "none", "flow": self.flow}]
                }]},
                "streamSettings": stream
            }, {"tag": "direct", "protocol": "freedom"}, {"tag": "block", "protocol": "blackhole"}],
            "routing": {"domainStrategy": "AsIs", "rules": rules.xray_rules()}
        })
    }
}

fn tun_config(endpoint: IpAddr, socks: SocketAddr, rules: &crate::routing::Rules) -> Value {
    let prefix = if endpoint.is_ipv4() { 32 } else { 128 };
    json!({
        "log": {"level": "info", "disabled": false},
        "dns": {
            "servers": [{
                "type": "tcp", "tag": "remote", "server": "1.1.1.1", "detour": "xray"
            }],
            "final": "remote"
        },
        "inbounds": [{
            "type": "tun", "tag": "tun-in",
            "address": ["172.28.237.1/30", "fdfe:237::1/126"],
            "mtu": 1400, "auto_route": true, "strict_route": true, "stack": "mixed",
            "dns_mode": "hijack",
            "route_exclude_address": [format!("{endpoint}/{prefix}")]
        }],
        "outbounds": [{
            "type": "socks", "tag": "xray",
            "server": socks.ip(), "server_port": socks.port(), "version": "5",
            "inet4_bind_address": "127.0.0.1"
        }, {"type": "direct", "tag": "direct"}],
        "route": {
            "auto_detect_interface": true,
            "rules": rules.tun_rules(),
            "final": if rules.fallback == crate::routing::Action::Direct { "direct" } else { "xray" }
        }
    })
}

fn binary(name: &str, variable: &str) -> PathBuf {
    if let Some(path) = env::var_os(variable) {
        return PathBuf::from(path);
    }
    let filename = format!("{name}{}", env::consts::EXE_SUFFIX);
    if let Some(path) = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join(&filename)))
        .filter(|path| path.is_file())
    {
        return path;
    }
    PathBuf::from(filename)
}

struct Process {
    child: Child,
    log: Arc<Mutex<ProcessLog>>,
    readers: Vec<thread::JoinHandle<()>>,
    name: &'static str,
}

#[derive(Default)]
struct ProcessLog {
    started: bool,
    recent: VecDeque<String>,
}

#[derive(Clone)]
struct LogSink {
    queue: mpsc::SyncSender<CoreLine>,
    dropped: Arc<AtomicUsize>,
    access_logs: bool,
}

fn collect_log(
    pipe: impl Read + Send + 'static,
    log: Arc<Mutex<ProcessLog>>,
    output: Option<(LogSink, Source, bool)>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(pipe);
        let mut buffer = Vec::new();
        while let Ok(1..) = reader.read_until(b'\n', &mut buffer) {
            let raw = String::from_utf8_lossy(&buffer);
            let raw = raw.trim_end_matches(['\r', '\n']);
            let line = if raw.len() > 4096 {
                format!("{}…", &raw[..raw.floor_char_boundary(4096)])
            } else {
                raw.to_owned()
            };
            buffer.clear();
            if line.is_empty() {
                continue;
            }
            let emit = output.as_ref().map(|(sink, source, stderr)| {
                (
                    sink,
                    CoreLine {
                        source: *source,
                        text: line.clone(),
                        stderr: *stderr,
                    },
                )
            });
            if emit.as_ref().is_some_and(|(sink, record)| {
                !sink.access_logs && record.level() == crate::logs::Level::Traffic
            }) {
                continue;
            }
            let mut log = log.lock().expect("process log lock");
            log.started |= line.contains("sing-box started (");
            if log.recent.len() == 3 {
                log.recent.pop_front();
            }
            log.recent.push_back(line);
            drop(log);
            if let Some((sink, record)) = emit
                && matches!(
                    sink.queue.try_send(record),
                    Err(mpsc::TrySendError::Full(_))
                )
            {
                sink.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    })
}

impl Process {
    fn spawn(
        name: &'static str,
        program: PathBuf,
        args: &[&str],
        config: &Path,
        sink: Option<LogSink>,
    ) -> Result<Self> {
        let mut command = Command::new(&program);
        if name == "sing-box" {
            command.env("NO_COLOR", "1");
        }
        command
            .args(args)
            .arg(config)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x00000200); // CREATE_NEW_PROCESS_GROUP
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("无法启动 {name}，请检查可执行文件位置"))?;
        let log = Arc::new(Mutex::new(ProcessLog::default()));
        let source = if name == "sing-box" {
            Source::SingBox
        } else {
            Source::Xray
        };
        let readers = vec![
            collect_log(
                child.stdout.take().expect("piped stdout"),
                log.clone(),
                sink.clone().map(|sink| (sink, source, false)),
            ),
            collect_log(
                child.stderr.take().expect("piped stderr"),
                log.clone(),
                sink.map(|sink| (sink, source, true)),
            ),
        ];
        Ok(Self {
            child,
            log,
            readers,
            name,
        })
    }

    fn started(&self) -> bool {
        self.log.lock().expect("process log lock").started
    }

    fn check(&mut self) -> Result<()> {
        if let Some(status) = self.child.try_wait()? {
            for reader in self.readers.drain(..) {
                let _ = reader.join();
            }
            let log = self.log.lock().expect("process log lock");
            let details = log
                .recent
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join("\n");
            bail!("{} 已退出（{}）\n{}", self.name, status, details);
        }
        Ok(())
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            interrupt(self.child.id());
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline && matches!(self.child.try_wait(), Ok(None)) {
                thread::sleep(Duration::from_millis(30));
            }
            if matches!(self.child.try_wait(), Ok(None)) {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
        for reader in self.readers.drain(..) {
            let _ = reader.join();
        }
    }
}

#[cfg(target_os = "macos")]
fn interrupt(pid: u32) {
    // The child is still owned by this process.
    unsafe { libc::kill(pid as libc::pid_t, libc::SIGINT) };
}

#[cfg(windows)]
fn interrupt(pid: u32) {
    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn GenerateConsoleCtrlEvent(event: u32, group_id: u32) -> i32;
    }
    // CTRL_BREAK_EVENT can be addressed to the child's console group.
    unsafe { GenerateConsoleCtrlEvent(1, pid) };
}

#[cfg(not(any(windows, target_os = "macos")))]
fn interrupt(_: u32) {}

fn cancelled(stop: &Receiver<()>, timeout: Duration) -> bool {
    !matches!(
        stop.recv_timeout(timeout),
        Err(mpsc::RecvTimeoutError::Timeout)
    )
}

fn check_socks(address: SocketAddr) -> std::io::Result<bool> {
    let timeout = Duration::from_millis(150);
    let mut stream = TcpStream::connect_timeout(&address, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    stream.write_all(&[5, 1, 0])?;
    let mut reply = [0; 2];
    stream.read_exact(&mut reply)?;
    Ok(reply == [5, 0])
}

fn resolve(node: &Node, stop: &Receiver<()>) -> Result<Option<IpAddr>> {
    if let Ok(ip) = node.host.parse() {
        return Ok(Some(ip));
    }
    let host = node.host.clone();
    let port = node.port;
    let (send, receive) = mpsc::channel();
    thread::spawn(move || {
        let address = (host.as_str(), port)
            .to_socket_addrs()
            .context("无法解析节点地址")
            .and_then(|mut addresses| addresses.next().context("节点域名没有可用地址"));
        let _ = send.send(address);
    });
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        match receive.try_recv() {
            Ok(address) => return Ok(Some(address?.ip())),
            Err(TryRecvError::Disconnected) => bail!("节点解析已停止"),
            Err(TryRecvError::Empty) => {}
        }
        ensure!(Instant::now() < deadline, "解析节点地址超时");
        if cancelled(stop, Duration::from_millis(50)) {
            return Ok(None);
        }
    }
}

fn run(
    node: Node,
    mode: Mode,
    rules: crate::routing::Rules,
    stop: &Receiver<()>,
    events: &Sender<Event>,
    logs: LogSink,
    settings: crate::settings::Settings,
) -> Result<()> {
    let Some(endpoint) = resolve(&node, stop)? else {
        return Ok(());
    };
    if cancelled(stop, Duration::ZERO) {
        return Ok(());
    }

    let mut temp = tempfile::Builder::new();
    temp.prefix("gpui-proxy-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temp.permissions(fs::Permissions::from_mode(0o700));
    }
    let dir = temp.tempdir()?;
    let reservation = TcpListener::bind(("127.0.0.1", 0))?;
    let socks = reservation.local_addr()?;
    let config = dir.path().join("xray.json");
    let unused = crate::routing::Rules::default();
    let local_rules = if mode == Mode::Tun { &unused } else { &rules };
    fs::write(
        &config,
        node.config(endpoint, socks, local_rules, settings.access_logs)
            .to_string(),
    )?;
    drop(reservation);
    let mut xray = Process::spawn(
        "Xray",
        settings
            .xray_path
            .unwrap_or_else(|| binary("xray", "GPUI_PROXY_XRAY")),
        &["run", "-config"],
        &config,
        Some(logs.clone()),
    )?;
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        xray.check()?;
        if check_socks(socks).unwrap_or(false) {
            break;
        }
        ensure!(Instant::now() < deadline, "等待 Xray SOCKS 入站超时");
        if cancelled(stop, Duration::from_millis(100)) {
            return Ok(());
        }
    }

    let mut tun = None;
    if mode == Mode::Tun {
        let config = dir.path().join("tun.json");
        fs::write(&config, tun_config(endpoint, socks, &rules).to_string())?;
        let mut process = Process::spawn(
            "sing-box",
            settings
                .sing_box_path
                .unwrap_or_else(|| binary("sing-box", "GPUI_PROXY_SING_BOX")),
            &["run", "-c"],
            &config,
            Some(logs),
        )?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            process.check()?;
            if process.started() {
                break;
            }
            ensure!(Instant::now() < deadline, "等待 TUN 网卡就绪超时");
            if cancelled(stop, Duration::from_millis(100)) {
                return Ok(());
            }
        }
        tun = Some(process);
    }
    if events.send(Event::Ready(socks)).is_err() {
        return Ok(());
    }
    while !cancelled(stop, Duration::from_millis(250)) {
        xray.check()?;
        if let Some(tun) = &mut tun {
            tun.check()?;
        }
    }
    Ok(())
}

pub struct Connection {
    stop: Sender<()>,
    pub events: Receiver<Event>,
    pub logs: Receiver<CoreLine>,
    dropped_logs: Arc<AtomicUsize>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Connection {
    pub fn start(
        node: Node,
        mode: Mode,
        rules: crate::routing::Rules,
        settings: crate::settings::Settings,
    ) -> Self {
        let (stop, request) = mpsc::channel();
        let (events, receive) = mpsc::channel();
        let (queue, logs) = mpsc::sync_channel(512);
        let dropped_logs = Arc::new(AtomicUsize::new(0));
        let sink = LogSink {
            queue,
            dropped: dropped_logs.clone(),
            access_logs: settings.access_logs,
        };
        let worker = thread::spawn(move || {
            let event = match run(node, mode, rules, &request, &events, sink, settings) {
                Ok(()) => Event::Stopped,
                Err(error) => Event::Failed(format!("{error:#}")),
            };
            let _ = events.send(event);
        });
        Self {
            stop,
            events: receive,
            logs,
            dropped_logs,
            worker: Some(worker),
        }
    }

    pub fn take_dropped_logs(&self) -> usize {
        self.dropped_logs.swap(0, Ordering::Relaxed)
    }

    pub fn stop(&self) {
        let _ = self.stop.send(());
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.stop();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
use crate::logs::{CoreLine, Source};
