mod dashboard_view;
mod logs;
mod proxy;
mod routing;
mod settings;
mod subscriptions;

use gpui::{
    App, Application, Bounds, Context, FocusHandle, Window, WindowBounds, WindowOptions,
    prelude::*, px, size,
};
use proxy::{Connection, Event, Mode, Node};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Overview,
    Nodes,
    Subscriptions,
    Routing,
    Logs,
    Settings,
}

struct Dashboard {
    focus: FocusHandle,
    page: Page,
    mode: Mode,
    nodes: Vec<Node>,
    manual_nodes: Vec<Node>,
    subscriptions: subscriptions::Manager,
    rules: routing::Rules,
    rules_path: Option<std::path::PathBuf>,
    rules_feedback: String,
    rules_failed: bool,
    socks: Option<std::net::SocketAddr>,
    selected: usize,
    logs: VecDeque<logs::Entry>,
    log_filter: logs::Filter,
    settings: settings::Settings,
    settings_path: Option<std::path::PathBuf>,
    settings_feedback: String,
    settings_failed: bool,
    startup_enabled: Option<bool>,
    started_at: Instant,
    connection: Option<Connection>,
    status: &'static str,
    detail: String,
    failed: bool,
}

impl Dashboard {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        let mut dashboard = Self {
            focus,
            page: Page::Overview,
            mode: Mode::Local,
            nodes: Vec::new(),
            manual_nodes: Vec::new(),
            subscriptions: subscriptions::Manager::new(if cfg!(test) {
                Err(anyhow::anyhow!("测试环境未设置订阅存储"))
            } else {
                subscriptions::storage_path()
            }),
            rules: routing::Rules::default(),
            rules_path: None,
            rules_feedback: String::new(),
            rules_failed: false,
            socks: None,
            selected: 0,
            logs: VecDeque::new(),
            log_filter: logs::Filter::All,
            settings: settings::Settings::default(),
            settings_path: None,
            settings_feedback: String::new(),
            settings_failed: false,
            startup_enabled: None,
            started_at: Instant::now(),
            connection: None,
            status: "待连接",
            detail: String::new(),
            failed: false,
        };
        if !cfg!(test) {
            match settings::path().and_then(|path| settings::load(&path).map(|value| (path, value)))
            {
                Ok((path, value)) => {
                    dashboard.mode = value.default_mode;
                    dashboard.settings = value;
                    dashboard.settings_path = Some(path);
                }
                Err(error) => dashboard.settings_error(error.to_string()),
            }
            dashboard.startup_enabled = settings::startup_enabled().ok();
            match routing::path().and_then(|path| routing::load(&path).map(|rules| (path, rules))) {
                Ok((path, rules)) => {
                    dashboard.rules_path = Some(path);
                    dashboard.rules = rules;
                }
                Err(error) => {
                    dashboard.rules_feedback = error.to_string();
                    dashboard.rules_failed = true;
                }
            }
        }
        dashboard.rebuild_nodes(None);
        dashboard
    }

    fn rebuild_nodes(&mut self, preferred: Option<Node>) {
        let preferred = preferred.or_else(|| self.node().cloned());
        let mut nodes = self.manual_nodes.clone();
        for node in self
            .subscriptions
            .records
            .iter()
            .flat_map(|record| &record.links)
            .filter_map(|link| Node::parse(link).ok())
        {
            if !nodes.contains(&node) {
                nodes.push(node);
            }
        }
        self.selected = preferred
            .and_then(|node| {
                nodes.iter().position(|item| item == &node).or_else(|| {
                    if self.connection.is_some() {
                        nodes.push(node);
                        Some(nodes.len() - 1)
                    } else {
                        None
                    }
                })
            })
            .unwrap_or(0);
        self.nodes = nodes;
    }

    fn subscription_proxy(&self) -> Option<std::net::SocketAddr> {
        self.socks
            .filter(|_| self.mode == Mode::Local && self.status == "本地代理已就绪")
    }

    fn node(&self) -> Option<&Node> {
        self.nodes.get(self.selected)
    }

    fn settings_error(&mut self, message: String) {
        self.settings_feedback = message;
        self.settings_failed = true;
    }

    fn edit_settings(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut settings::Settings),
    ) {
        let Some(path) = self.settings_path.as_deref() else {
            return;
        };
        let mut next = self.settings.clone();
        edit(&mut next);
        match settings::save(path, &self.settings, &next) {
            Ok(()) => {
                if self.connection.is_none() && next.default_mode != self.settings.default_mode {
                    self.mode = next.default_mode;
                }
                let restart_required = self.connection.is_some()
                    && (next.access_logs != self.settings.access_logs
                        || next.xray_path != self.settings.xray_path
                        || next.sing_box_path != self.settings.sing_box_path);
                self.settings = next;
                self.settings_feedback = if restart_required {
                    "已保存，核心和日志设置在重新连接后生效".into()
                } else {
                    "已保存设置".into()
                };
                self.settings_failed = false;
            }
            Err(error) => self.settings_error(format!("{error:#}")),
        }
        cx.notify();
    }

    fn choose_core(&mut self, core: settings::Core, cx: &mut Context<Self>) {
        let picker = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(format!("选择 {} 可执行文件", core.label()).into()),
        });
        cx.spawn(async move |view, cx| {
            let picked = picker.await.ok().transpose().and_then(|items| {
                items
                    .flatten()
                    .and_then(|paths| paths.into_iter().next())
                    .map(settings::validate_executable)
                    .transpose()
            });
            let _ = view.update(cx, |view, cx| match picked {
                Ok(Some(path)) => view.edit_settings(cx, |value| core.set_path(value, Some(path))),
                Ok(None) => {}
                Err(error) => {
                    view.settings_error(format!("{error:#}"));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn toggle_startup(&mut self, cx: &mut Context<Self>) {
        let Some(enabled) = self.startup_enabled else {
            return;
        };
        match settings::set_startup(!enabled).and_then(|()| settings::startup_enabled()) {
            Ok(current) => {
                self.startup_enabled = Some(current);
                self.settings_failed = false;
                self.settings_feedback = if current {
                    "已启用登录时启动".into()
                } else {
                    "已关闭登录时启动".into()
                };
            }
            Err(error) => self.settings_error(error.to_string()),
        }
        cx.notify();
    }

    fn edit_rules(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut routing::Rules) -> anyhow::Result<()>,
    ) {
        let Some(path) = self.rules_path.as_deref() else {
            return;
        };
        let mut updated = self.rules.clone();
        match edit(&mut updated).and_then(|()| routing::save(path, &self.rules, &updated)) {
            Ok(()) => {
                self.rules = updated;
                self.rules_feedback = if self.connection.is_some() {
                    "已保存，重新连接后生效".into()
                } else {
                    "已保存路由规则".into()
                };
                self.rules_failed = false;
            }
            Err(error) => {
                self.rules_feedback = format!("{error:#}");
                self.rules_failed = true;
            }
        }
        cx.notify();
    }

    fn record(&mut self, title: &'static str, detail: String, failed: bool) {
        let level = if failed {
            logs::Level::Error
        } else {
            logs::Level::Info
        };
        self.push_log(logs::Source::Client, level, title, detail);
    }

    fn push_log(
        &mut self,
        source: logs::Source,
        level: logs::Level,
        title: &'static str,
        detail: String,
    ) {
        self.logs.push_front(logs::Entry {
            elapsed: self.started_at.elapsed(),
            source,
            level,
            title,
            detail,
        });
        self.logs.truncate(240);
    }

    fn clear_logs(&mut self) {
        self.logs.clear();
        if let Some(connection) = &self.connection {
            for _ in connection.logs.try_iter().take(512) {}
            connection.take_dropped_logs();
        }
    }

    fn log_text(&self) -> String {
        use std::fmt::Write;
        let mut text = String::new();
        for entry in self
            .logs
            .iter()
            .rev()
            .filter(|entry| self.log_filter.accepts(entry))
        {
            let _ = writeln!(
                text,
                "{} [{}] [{}] {}{}{}",
                entry.time(),
                entry.source.label(),
                entry.level.label(),
                entry.title,
                if entry.title.is_empty() || entry.detail.is_empty() {
                    ""
                } else {
                    " — "
                },
                entry.detail
            );
        }
        text
    }

    fn import(&mut self, cx: &mut Context<Self>) {
        let result = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .ok_or_else(|| anyhow::anyhow!("剪贴板中没有文本"))
            .and_then(|text| Node::parse(&text));
        match result {
            Ok(node) => {
                let label = node.label.clone();
                if !self.manual_nodes.contains(&node) {
                    self.manual_nodes.push(node.clone());
                }
                self.rebuild_nodes(Some(node));
                self.record("已导入节点", label, false);
                self.status = "待连接";
                self.detail.clear();
                self.failed = false;
            }
            Err(error) => self.show_error(format!("{error:#}")),
        }
        cx.notify();
    }

    fn show_error(&mut self, detail: String) {
        self.status = "遇到错误";
        self.record("操作失败", detail.clone(), true);
        self.detail = detail;
        self.failed = true;
    }

    fn toggle(&mut self, cx: &mut Context<Self>) {
        if let Some(connection) = &self.connection {
            connection.stop();
            self.status = "正在断开";
            self.detail.clear();
            cx.notify();
            return;
        }
        let Some(node) = self.node().cloned() else {
            return;
        };
        self.record("正在启动核心", node.label.clone(), false);
        self.connection = Some(Connection::start(
            node,
            self.mode,
            self.rules.clone(),
            self.settings.clone(),
        ));
        self.status = "正在启动";
        self.detail.clear();
        self.failed = false;
        cx.notify();
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
        let (changed, rebuild) = self
            .subscriptions
            .poll(self.subscription_proxy(), self.settings.auto_refresh);
        if rebuild {
            self.rebuild_nodes(None);
        }
        if changed {
            cx.notify();
        }
        let Some(connection) = &self.connection else {
            return;
        };
        let event = connection.events.try_recv();
        let lines: Vec<_> = connection.logs.try_iter().take(512).collect();
        let dropped = connection.take_dropped_logs();
        if !lines.is_empty() || dropped > 0 {
            for line in lines {
                self.push_log(line.source, line.level(), "", line.text);
            }
            if dropped > 0 {
                self.push_log(
                    logs::Source::Client,
                    logs::Level::Warning,
                    "核心日志过载",
                    format!("跳过了 {dropped} 条日志"),
                );
            }
            cx.notify();
        }
        match event {
            Ok(Event::Ready(address)) if self.status != "正在断开" => {
                self.socks = Some(address);
                self.status = "本地代理已就绪";
                if self.mode == Mode::Tun {
                    self.status = "TUN 已启动";
                    self.detail = "TUN 流量通过 Xray 转发".into();
                } else {
                    self.detail = format!("SOCKS5  {address}");
                }
                self.record(self.status, self.detail.clone(), false);
            }
            Ok(Event::Stopped) => {
                self.connection = None;
                self.socks = None;
                self.rebuild_nodes(None);
                self.status = "已断开";
                self.detail.clear();
                self.record("连接已断开", String::new(), false);
            }
            Ok(Event::Failed(error)) => {
                self.connection = None;
                self.socks = None;
                self.rebuild_nodes(None);
                self.show_error(error);
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.connection = None;
                self.socks = None;
                self.rebuild_nodes(None);
                self.show_error("连接管理进程已停止".into());
            }
            _ => return,
        }
        cx.notify();
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let preferred = size(px(1350.), px(880.));
        let initial_size = cx.primary_display().map_or(preferred, |display| {
            let screen = display.bounds().size;
            size(
                preferred.width.min(screen.width - px(48.)).max(px(820.)),
                preferred.height.min(screen.height - px(84.)).max(px(540.)),
            )
        });
        let bounds = Bounds::centered(None, initial_size, cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Xray Connect".into()),
                    appears_transparent: cfg!(windows),
                    traffic_light_position: None,
                }),
                window_min_size: Some(size(px(820.), px(540.))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Xray Connect");
                cx.new(|cx: &mut Context<Dashboard>| {
                    cx.on_app_quit(|view, _| {
                        drop(view.connection.take());
                        std::future::ready(())
                    })
                    .detach();
                    cx.spawn(async |view, cx| {
                        loop {
                            cx.background_executor()
                                .timer(Duration::from_millis(120))
                                .await;
                            if view.update(cx, |view, cx| view.poll(cx)).is_err() {
                                break;
                            }
                        }
                    })
                    .detach();
                    Dashboard::new(window, cx)
                })
            },
        )
        .expect("无法打开 GPUI 窗口");
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}
