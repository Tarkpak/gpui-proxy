use crate::{Dashboard, Mode, Page};
use gpui::{
    Context, Div, FontWeight, IntoElement, Render, SharedString, Stateful, Window, div,
    linear_color_stop, linear_gradient, prelude::*, px, rgb,
};

const BG: u32 = 0x10141d;
const RAIL: u32 = 0x171d28;
const PANEL: u32 = 0x191f2a;
const EDGE: u32 = 0x343d4c;
const TEXT: u32 = 0xf0f3fa;
const MUTED: u32 = 0xa8b2c4;
const DIM: u32 = 0x7c889c;
const GREEN: u32 = 0x46e69a;
const BLUE: u32 = 0x35b7f3;
const PURPLE: u32 = 0xa88aff;

fn column() -> Div {
    div().flex().flex_col().min_w_0()
}

fn row() -> Div {
    div().flex().items_center().min_w_0()
}

fn small(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(12.))
        .text_color(rgb(MUTED))
        .child(text.into())
}

fn title(text: &'static str) -> Div {
    div()
        .text_size(px(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
}

fn dot(color: u32) -> Div {
    div()
        .size(px(7.))
        .flex_shrink_0()
        .rounded_full()
        .bg(rgb(color))
}

fn panel() -> Div {
    column()
        .p(px(15.))
        .gap(px(13.))
        .rounded(px(12.))
        .bg(rgb(PANEL))
        .border_1()
        .border_color(rgb(EDGE))
}

fn button(id: &'static str, name: &'static str, enabled: bool) -> Stateful<Div> {
    row()
        .id(id)
        .debug_selector(|| id.to_owned())
        .justify_center()
        .flex_shrink_0()
        .h(px(34.))
        .px(px(12.))
        .rounded(px(7.))
        .border_1()
        .border_color(rgb(EDGE))
        .bg(rgb(0x242c3a))
        .text_size(px(12.))
        .text_color(rgb(if enabled { TEXT } else { DIM }))
        .child(name)
        .when(enabled, |control| {
            control
                .cursor_pointer()
                .tab_index(0)
                .hover(|style| style.bg(rgb(0x303c50)))
                .active(|style| style.bg(rgb(0x202636)))
                .focus(|style| style.border_color(rgb(PURPLE)))
        })
}

fn switch(on: bool) -> Div {
    row()
        .w(px(38.))
        .h(px(21.))
        .p(px(3.))
        .rounded_full()
        .flex_shrink_0()
        .bg(rgb(if on { 0x29ca78 } else { 0x454e5b }))
        .when(on, |control| control.justify_end())
        .child(div().size(px(15.)).rounded_full().bg(rgb(0xf7f9fd)))
}

fn nav_item(
    index: usize,
    glyph: &'static str,
    label: &'static str,
    page: Option<Page>,
    view: &Dashboard,
    cx: &mut Context<Dashboard>,
) -> Stateful<Div> {
    let selected = page == Some(view.page);
    row()
        .id(("nav", index))
        .debug_selector(|| label.to_owned())
        .h(px(42.))
        .gap(px(14.))
        .px(px(14.))
        .rounded(px(8.))
        .border_1()
        .border_color(rgb(if selected { 0x7166d8 } else { RAIL }))
        .bg(rgb(if selected { 0x443d97 } else { RAIL }))
        .text_color(rgb(if page.is_some() { TEXT } else { DIM }))
        .child(div().w(px(19.)).text_size(px(17.)).child(glyph))
        .child(
            div()
                .text_size(px(14.))
                .font_weight(FontWeight::MEDIUM)
                .child(label),
        )
        .when(page.is_some(), |entry| {
            entry
                .cursor_pointer()
                .tab_index(0)
                .hover(|style| style.bg(rgb(if selected { 0x5149ae } else { 0x242c39 })))
                .focus(|style| style.border_color(rgb(PURPLE)))
        })
        .on_click(cx.listener(move |this, _, _, cx| {
            if let Some(page) = page {
                this.page = page;
                cx.notify();
            }
        }))
}

fn sidebar(view: &Dashboard, compact: bool, cx: &mut Context<Dashboard>) -> Div {
    let ready = view.ready();
    column()
        .w(px(if compact { 158. } else { 196. }))
        .h_full()
        .flex_shrink_0()
        .bg(rgb(RAIL))
        .border_r_1()
        .border_color(rgb(EDGE))
        .px(px(12.))
        .child(
            row()
                .h(px(70.))
                .gap(px(9.))
                .px(px(6.))
                .child(
                    div()
                        .text_size(px(32.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(BLUE))
                        .child("X"),
                )
                .child(
                    div()
                        .text_size(px(17.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Xray Connect"),
                ),
        )
        .child(
            column().gap(px(5.)).pt(px(8.)).children(
                [
                    ("⌂", "概览", Some(Page::Overview)),
                    ("▤", "节点", Some(Page::Nodes)),
                    ("◔", "订阅", Some(Page::Subscriptions)),
                    ("⌘", "路由规则", Some(Page::Routing)),
                    ("≡", "连接日志", Some(Page::Logs)),
                    ("⚙", "设置", Some(Page::Settings)),
                ]
                .into_iter()
                .enumerate()
                .map(|(index, (glyph, label, page))| nav_item(index, glyph, label, page, view, cx)),
            ),
        )
        .child(div().flex_1())
        .child(
            column()
                .mx(px(6.))
                .py(px(18.))
                .gap(px(9.))
                .border_t_1()
                .border_color(rgb(EDGE))
                .child(title("Xray 内核"))
                .child(
                    row()
                        .gap(px(8.))
                        .child(dot(if ready { GREEN } else { DIM }))
                        .child(small(if ready {
                            "运行中"
                        } else if view.connection.is_some() {
                            "正在切换状态"
                        } else {
                            "未运行"
                        })),
                ),
        )
}

fn header(page: Page) -> Div {
    let (name, description) = match page {
        Page::Overview => ("概览", "连接状态、网络流量与节点信息一览"),
        Page::Nodes => ("节点", "导入 VLESS 链接并选择连接节点"),
        Page::Subscriptions => ("订阅", "从订阅源获取和更新代理节点"),
        Page::Routing => ("路由规则", "决定匹配流量的转发方式"),
        Page::Logs => ("连接日志", "当前会话的代理启动、断开与错误记录"),
        Page::Settings => ("设置", "管理默认接入、代理核心和后台行为"),
    };
    row()
        .h(px(70.))
        .flex_shrink_0()
        .gap(px(16.))
        .pl(px(20.))
        .border_b_1()
        .border_color(rgb(EDGE))
        .bg(rgb(0x151a24))
        .child(
            column()
                .debug_selector(|| "header-copy".into())
                .flex_1()
                .h_full()
                .justify_center()
                .window_control_area(gpui::WindowControlArea::Drag)
                .gap(px(3.))
                .child(
                    div()
                        .text_size(px(23.))
                        .line_height(px(28.))
                        .whitespace_nowrap()
                        .font_weight(FontWeight::BOLD)
                        .child(name),
                )
                .child(
                    small(description)
                        .line_height(px(17.))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis(),
                ),
        )
        .when(page != Page::Subscriptions, |header| {
            header.child(
                row()
                    .debug_selector(|| "header-config".into())
                    .gap(px(10.))
                    .flex_shrink_0()
                    .window_control_area(gpui::WindowControlArea::Drag)
                    .child(small("当前配置").whitespace_nowrap())
                    .child(
                        div()
                            .px(px(12.))
                            .py(px(8.))
                            .rounded(px(7.))
                            .border_1()
                            .border_color(rgb(EDGE))
                            .text_size(px(12.))
                            .whitespace_nowrap()
                            .child("默认配置"),
                    ),
            )
        })
        .when(cfg!(windows), |bar| bar.child(window_controls()))
}

fn window_controls() -> Div {
    use gpui::WindowControlArea::{Close, Max, Min};
    row().flex_shrink_0().children(
        [
            ("−", Min, 0x282f3d),
            ("□", Max, 0x282f3d),
            ("×", Close, 0xba343b),
        ]
        .into_iter()
        .map(|(glyph, area, hover)| {
            row()
                .id(glyph)
                .w(px(42.))
                .h(px(36.))
                .justify_center()
                .text_size(px(17.))
                .window_control_area(area)
                .hover(move |style| style.bg(rgb(hover)))
                .child(glyph)
        }),
    )
}

fn connection(view: &Dashboard, cx: &mut Context<Dashboard>) -> Div {
    let ready = view.ready();
    let running = view.connection.is_some();
    let enabled = view.node().is_some() && view.status != "正在断开";
    let color = if ready {
        GREEN
    } else if view.failed {
        0xff898d
    } else {
        MUTED
    };
    let subtitle = if view.detail.is_empty() {
        if view.node().is_some() {
            "启动 Xray 并建立本地代理入口".into()
        } else {
            "导入 VLESS 节点后即可连接".into()
        }
    } else {
        view.detail.clone()
    };
    panel()
        .flex_1()
        .justify_between()
        .min_h(px(154.))
        .border_color(rgb(if ready { 0x34bf82 } else { 0x3a625b }))
        .bg(linear_gradient(
            125.,
            linear_color_stop(rgb(0x1b3b36), 0.),
            linear_color_stop(rgb(0x15252b), 1.),
        ))
        .child(
            row()
                .gap(px(12.))
                .child(
                    row()
                        .id("connection")
                        .debug_selector(|| "connection".into())
                        .w(px(65.))
                        .h(px(36.))
                        .p(px(4.))
                        .flex_shrink_0()
                        .rounded_full()
                        .bg(rgb(if ready {
                            0x23d982
                        } else if enabled {
                            0x445461
                        } else {
                            0x31444b
                        }))
                        .border_1()
                        .border_color(rgb(if ready { 0x41ec9a } else { 0x52656d }))
                        .when(running, |control| control.justify_end())
                        .child(
                            div()
                                .size(px(26.))
                                .rounded_full()
                                .bg(rgb(if ready || enabled { 0xf3f6fb } else { 0x778a90 })),
                        )
                        .when(enabled, |control| {
                            control
                                .cursor_pointer()
                                .tab_index(0)
                                .hover(move |style| {
                                    style.border_color(rgb(if ready { 0xa0ffd0 } else { 0x94a5b3 }))
                                })
                                .focus(|style| style.border_color(rgb(PURPLE)))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if enabled {
                                this.toggle(cx);
                            }
                        })),
                )
                .child(
                    column()
                        .gap(px(4.))
                        .child(
                            div()
                                .text_size(px(22.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(color))
                                .child(if !running && !view.failed {
                                    "代理未连接"
                                } else {
                                    view.status
                                }),
                        )
                        .child(
                            small(subtitle)
                                .text_size(px(13.))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis(),
                        ),
                ),
        )
        .child(
            row()
                .gap(px(14.))
                .pt(px(11.))
                .border_t_1()
                .border_color(rgb(0x42615e))
                .child(
                    column()
                        .flex_1()
                        .gap(px(4.))
                        .child(small("当前节点"))
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(view.node().map_or_else(
                                    || "未选择节点".into(),
                                    |node| node.label.clone(),
                                )),
                        ),
                )
                .child(
                    column()
                        .gap(px(4.))
                        .flex_shrink_0()
                        .pl(px(12.))
                        .border_l_1()
                        .border_color(rgb(0x42615e))
                        .child(small("接入模式"))
                        .child(div().text_size(px(13.)).child(if view.mode == Mode::Tun {
                            "TUN"
                        } else {
                            "SOCKS5"
                        })),
                ),
        )
}

fn metric(label: &'static str, mark: &'static str, tint: u32, color: u32) -> Div {
    column()
        .flex_1()
        .min_h(px(72.))
        .px(px(10.))
        .py(px(9.))
        .justify_between()
        .rounded(px(9.))
        .bg(rgb(tint))
        .border_1()
        .border_color(rgb(EDGE))
        .child(
            row()
                .gap(px(6.))
                .child(div().text_size(px(14.)).text_color(rgb(color)).child(mark))
                .child(small(label)),
        )
        .child(
            div()
                .text_size(px(22.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(color))
                .child("—"),
        )
}

fn metrics() -> Div {
    column()
        .w(px(224.))
        .flex_shrink_0()
        .gap(px(9.))
        .child(
            row()
                .gap(px(9.))
                .child(metric("上传速度", "↑", 0x28253b, PURPLE))
                .child(metric("下载速度", "↓", 0x1a2a3c, BLUE)),
        )
        .child(
            row()
                .gap(px(9.))
                .child(metric("累计流量", "◉", 0x183037, 0x6ed9de))
                .child(metric("活跃连接", "⌘", 0x25293d, 0xc1b6f8)),
        )
}

fn legend(name: &'static str, color: u32) -> Div {
    row().gap(px(6.)).child(dot(color)).child(small(name))
}

fn traffic(short: bool) -> Div {
    panel()
        .child(
            row().justify_between().child(title("实时网络流量")).child(
                row()
                    .gap(px(12.))
                    .child(legend("下载", BLUE))
                    .child(legend("上传", PURPLE)),
            ),
        )
        .child(
            div()
                .relative()
                .h(px(if short { 128. } else { 180. }))
                .rounded(px(6.))
                .overflow_hidden()
                .bg(rgb(0x131b27))
                .child(column().absolute().inset_0().children(
                    (0..5).map(|_| div().flex_1().border_b_1().border_color(rgb(0x263143))),
                ))
                .child(div().flex().absolute().inset_0().children(
                    (0..8).map(|_| div().flex_1().border_r_1().border_color(rgb(0x263143))),
                ))
                .child(
                    row().absolute().inset_0().justify_center().child(
                        div()
                            .px(px(12.))
                            .py(px(7.))
                            .rounded(px(6.))
                            .bg(rgb(0x192332))
                            .child(small("流量统计尚未接入")),
                    ),
                ),
        )
}

fn cell(text: impl Into<SharedString>) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .text_size(px(12.))
        .child(text.into())
}

fn nodes(view: &Dashboard, cx: &mut Context<Dashboard>) -> Div {
    let idle = view.connection.is_none();
    let mut card = panel()
        .child(
            row()
                .justify_between()
                .gap(px(8.))
                .child(title("节点列表"))
                .child(
                    button("import", "＋ 从剪贴板导入", idle).on_click(cx.listener(
                        move |this, _, _, cx| {
                            if idle {
                                this.import(cx);
                            }
                        },
                    )),
                ),
        )
        .child(
            row()
                .gap(px(8.))
                .px(px(10.))
                .h(px(32.))
                .rounded(px(5.))
                .bg(rgb(0x232a37))
                .text_color(rgb(MUTED))
                .child(cell("节点地址").flex_grow())
                .child(cell("协议"))
                .child(div().w(px(66.)).text_size(px(12.)).child("操作")),
        );
    if view.nodes.is_empty() {
        return card.child(
            column()
                .min_h(px(112.))
                .items_center()
                .justify_center()
                .gap(px(7.))
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .child("暂未导入节点"),
                )
                .child(small("复制 VLESS 分享链接，再从剪贴板导入。")),
        );
    }
    for (index, node) in view.nodes.iter().enumerate() {
        let selected = index == view.selected;
        let entry = row()
            .id(("node", index))
            .debug_selector(|| format!("node-{index}"))
            .gap(px(8.))
            .px(px(10.))
            .h(px(39.))
            .rounded(px(5.))
            .border_1()
            .border_color(rgb(if selected { 0x7a6bdf } else { 0x29303a }))
            .bg(rgb(if selected { 0x302a56 } else { PANEL }))
            .child(cell(node.label.clone()).flex_grow())
            .child(cell("VLESS · TCP"))
            .child(
                div()
                    .w(px(66.))
                    .text_size(px(12.))
                    .text_color(rgb(if selected { PURPLE } else { MUTED }))
                    .child(if selected {
                        "✓ 当前"
                    } else if idle {
                        "选择"
                    } else {
                        "连接中"
                    }),
            )
            .when(idle, |entry| {
                entry
                    .cursor_pointer()
                    .tab_index(0)
                    .hover(|style| style.bg(rgb(0x373154)))
                    .focus(|style| style.border_color(rgb(PURPLE)))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                if idle {
                    this.selected = index;
                    cx.notify();
                }
            }));
        card = card.child(entry);
    }
    card
}

fn quick_row(name: &'static str, desc: &'static str, mark: &'static str, on: bool) -> Div {
    row()
        .gap(px(10.))
        .min_h(px(48.))
        .py(px(5.))
        .border_t_1()
        .border_color(rgb(0x2f3644))
        .child(
            div()
                .w(px(20.))
                .flex_shrink_0()
                .text_size(px(17.))
                .text_color(rgb(MUTED))
                .child(mark),
        )
        .child(
            column()
                .flex_1()
                .gap(px(3.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(name),
                )
                .child(small(desc).text_size(px(11.))),
        )
        .child(switch(on))
}

fn controls(view: &Dashboard, cx: &mut Context<Dashboard>) -> Div {
    let tun = view.mode == Mode::Tun;
    let idle = view.connection.is_none();
    panel()
        .child(title("快捷控制"))
        .child(
            column()
                .child(quick_row("系统代理", "尚未接入", "▣", false))
                .child(
                    quick_row("TUN 模式", "虚拟网卡接管流量", "⌘", tun)
                        .id("tun")
                        .debug_selector(|| "tun".into())
                        .rounded(px(4.))
                        .when(idle, |entry| {
                            entry
                                .cursor_pointer()
                                .tab_index(0)
                                .hover(|style| style.bg(rgb(0x242c3b)))
                                .active(|style| style.bg(rgb(0x30394a)))
                                .focus(|style| style.bg(rgb(0x333052)))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if idle {
                                this.mode = if tun { Mode::Local } else { Mode::Tun };
                                cx.notify();
                            }
                        })),
                )
                .child(startup_control(view, cx)),
        )
        .child(
            column()
                .gap(px(9.))
                .pt(px(11.))
                .border_t_1()
                .border_color(rgb(EDGE))
                .child(title("接入方式"))
                .child(
                    row()
                        .gap(px(4.))
                        .p(px(3.))
                        .rounded(px(7.))
                        .border_1()
                        .border_color(rgb(EDGE))
                        .children([("SOCKS5", !tun), ("TUN", tun)].into_iter().map(
                            |(name, active)| {
                                row()
                                    .flex_1()
                                    .justify_center()
                                    .h(px(30.))
                                    .rounded(px(5.))
                                    .bg(rgb(if active { 0x5145b3 } else { PANEL }))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(rgb(if active { TEXT } else { MUTED }))
                                            .child(name),
                                    )
                            },
                        )),
                )
                .child(small(if tun {
                    "启动 TUN 需要管理员权限。"
                } else {
                    "为需要代理的应用配置 SOCKS5。"
                })),
        )
}

fn startup_control(view: &Dashboard, cx: &mut Context<Dashboard>) -> Stateful<Div> {
    let available = view.startup_enabled.is_some();
    quick_row(
        "开机启动",
        if available {
            "登录后打开客户端"
        } else {
            "当前环境不可用"
        },
        "⏻",
        view.startup_enabled == Some(true),
    )
    .id("startup")
    .when(available, |entry| {
        entry
            .cursor_pointer()
            .tab_index(0)
            .hover(|style| style.bg(rgb(0x242c3b)))
            .focus(|style| style.bg(rgb(0x333052)))
    })
    .on_click(cx.listener(move |this, _, _, cx| {
        if available {
            this.toggle_startup(cx);
            if this.settings_failed {
                this.page = Page::Settings;
                cx.notify();
            }
        }
    }))
}

fn settings_page(view: &Dashboard, cx: &mut Context<Dashboard>) -> Div {
    let available = view.settings_path.is_some();
    let general = panel()
        .child(title("常规"))
        .child(
            row()
                .justify_between()
                .gap(px(8.))
                .py(px(6.))
                .child(
                    column()
                        .gap(px(6.))
                        .child(title("默认接入方式"))
                        .child(small("启动客户端时选择的模式。TUN 需要管理员权限。")),
                )
                .child(
                    row().gap(px(7.)).children(
                        [
                            (Mode::Local, "settings-local", "SOCKS5"),
                            (Mode::Tun, "settings-tun", "TUN"),
                        ]
                        .into_iter()
                        .map(|(mode, id, label)| {
                            button(id, label, available)
                                .when(view.settings.default_mode == mode, |control| {
                                    control.bg(rgb(0x5145b3))
                                })
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if available {
                                        view.edit_settings(cx, |settings| {
                                            settings.default_mode = mode
                                        });
                                    }
                                }))
                        }),
                    ),
                ),
        )
        .child(startup_control(view, cx));
    let cores = panel()
        .child(title("代理核心"))
        .child(small(
            "留空时会按环境变量、程序同目录和系统 PATH 顺序查找。",
        ))
        .children(
            [crate::settings::Core::Xray, crate::settings::Core::SingBox]
                .into_iter()
                .enumerate()
                .map(|(index, core)| {
                    let path = core.path(&view.settings);
                    let configured = path.is_some();
                    row()
                        .gap(px(10.))
                        .py(px(8.))
                        .border_t_1()
                        .border_color(rgb(EDGE))
                        .child(
                            column()
                                .flex_1()
                                .gap(px(5.))
                                .child(title(core.label()))
                                .child(
                                    small(path.as_ref().map_or_else(
                                        || "自动查找".into(),
                                        |path| path.display().to_string(),
                                    ))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis(),
                                ),
                        )
                        .child(
                            button("choose-core", "选择文件", available)
                                .id(("choose-core", index))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if available {
                                        view.choose_core(core, cx);
                                    }
                                })),
                        )
                        .child(
                            button("restore-core", "自动查找", available && configured)
                                .id(("restore-core", index))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if available && configured {
                                        view.edit_settings(cx, |settings| {
                                            core.set_path(settings, None)
                                        });
                                    }
                                })),
                        )
                }),
        );
    let behavior = panel()
        .child(title("订阅与日志"))
        .child(
            quick_row(
                "自动刷新订阅",
                "检查超过 24 小时未更新的订阅",
                "↻",
                view.settings.auto_refresh,
            )
            .id("settings-refresh")
            .debug_selector(|| "settings-refresh".into())
            .when(available, |entry| {
                entry
                    .cursor_pointer()
                    .tab_index(0)
                    .hover(|style| style.bg(rgb(0x242c3b)))
                    .focus(|style| style.bg(rgb(0x333052)))
            })
            .on_click(cx.listener(move |view, _, _, cx| {
                if available {
                    view.edit_settings(cx, |settings| {
                        settings.auto_refresh = !settings.auto_refresh
                    });
                }
            })),
        )
        .child(
            quick_row(
                "访问日志",
                "记录流量目的地址，重新连接后生效",
                "≡",
                view.settings.access_logs,
            )
            .id("settings-access")
            .debug_selector(|| "settings-access".into())
            .when(available, |entry| {
                entry
                    .cursor_pointer()
                    .tab_index(0)
                    .hover(|style| style.bg(rgb(0x242c3b)))
                    .focus(|style| style.bg(rgb(0x333052)))
            })
            .on_click(cx.listener(move |view, _, _, cx| {
                if available {
                    view.edit_settings(cx, |settings| settings.access_logs = !settings.access_logs);
                }
            })),
        );
    column()
        .w_full()
        .max_w(px(1140.))
        .mx_auto()
        .p(px(16.))
        .gap(px(15.))
        .child(general)
        .child(cores)
        .child(behavior)
        .child(
            row()
                .justify_between()
                .gap(px(10.))
                .child(small(view.settings_feedback.clone()).text_color(rgb(
                    if view.settings_failed {
                        0xffa0a3
                    } else {
                        MUTED
                    },
                )))
                .child(
                    button("reset-settings", "恢复应用默认设置", available).on_click(cx.listener(
                        move |view, _, _, cx| {
                            if available {
                                view.edit_settings(cx, |settings| {
                                    *settings = crate::settings::Settings::default()
                                });
                            }
                        },
                    )),
                ),
        )
}

fn logs(view: &Dashboard, full: bool, cx: &mut Context<Dashboard>) -> Div {
    use crate::logs::{Filter, Level};
    let any = !view.logs.is_empty();
    let visible = view
        .logs
        .iter()
        .filter(|entry| !full || view.log_filter.accepts(entry));
    let count = visible.clone().count();
    let mut card = panel().min_h(px(190.)).child(
        row()
            .justify_between()
            .child(title(if full {
                "会话日志"
            } else {
                "最近连接日志"
            }))
            .when(!full, |line| {
                line.child(button("all-logs", "查看全部", true).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.page = Page::Logs;
                        cx.notify();
                    },
                )))
            }),
    );
    if full {
        card = card.child(
            row()
                .flex_wrap()
                .justify_between()
                .gap(px(8.))
                .child(
                    row()
                        .gap(px(6.))
                        .children(Filter::ALL.into_iter().enumerate().map(|(index, filter)| {
                            button(filter.label(), filter.label(), true)
                                .id(("log-filter", index))
                                .when(view.log_filter == filter, |control| {
                                    control.bg(rgb(0x5145b3))
                                })
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.log_filter = filter;
                                    cx.notify();
                                }))
                        })),
                )
                .child(
                    row()
                        .gap(px(7.))
                        .child(small(format!("{count} / {} 条", view.logs.len())))
                        .child(button("copy-logs", "复制筛选结果", count > 0).on_click(
                            cx.listener(move |view, _, _, cx| {
                                if count > 0 {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                        view.log_text(),
                                    ));
                                }
                            }),
                        ))
                        .child(button("clear-logs", "清空", any).on_click(cx.listener(
                            move |view, _, _, cx| {
                                if any {
                                    view.clear_logs();
                                    cx.notify();
                                }
                            },
                        ))),
                ),
        );
    }
    if count == 0 {
        return card.child(
            column()
                .min_h(px(96.))
                .gap(px(8.))
                .justify_center()
                .child(div().text_size(px(13.)).child(if any {
                    "没有匹配的记录"
                } else {
                    "暂无日志"
                }))
                .child(small(if any {
                    "切换筛选可查看其他记录。"
                } else {
                    "连接事件和核心输出会显示在这里。"
                })),
        );
    }
    for entry in visible.take(if full { 240 } else { 8 }) {
        let color = match entry.level {
            Level::Info => MUTED,
            Level::Traffic => BLUE,
            Level::Warning => 0xf4bd6c,
            Level::Error => 0xff8b91,
        };
        let fields = row()
            .gap(px(7.))
            .child(small(entry.time()).w(px(57.)).flex_shrink_0())
            .child(
                div()
                    .px(px(6.))
                    .py(px(3.))
                    .rounded(px(4.))
                    .bg(rgb(0x252e40))
                    .text_size(px(11.))
                    .text_color(rgb(color))
                    .child(entry.level.label()),
            )
            .child(small(entry.source.label()).flex_shrink_0());
        if full {
            let detail = if entry.title.is_empty() || entry.detail.is_empty() {
                entry.message().to_owned()
            } else {
                format!("{} · {}", entry.title, entry.detail)
            };
            card = card.child(
                row()
                    .gap(px(12.))
                    .py(px(6.))
                    .border_t_1()
                    .border_color(rgb(0x2e3542))
                    .child(fields.w(px(192.)).flex_shrink_0())
                    .child(small(detail).flex_1()),
            );
        } else {
            card = card.child(
                column()
                    .gap(px(5.))
                    .pt(px(7.))
                    .border_t_1()
                    .border_color(rgb(0x2e3542))
                    .child(fields)
                    .child(
                        small(
                            (if entry.title.is_empty() {
                                entry.message()
                            } else {
                                entry.title
                            })
                            .to_owned(),
                        )
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis(),
                    ),
            );
        }
    }
    card
}

fn routing_page(view: &Dashboard, cx: &mut Context<Dashboard>) -> Div {
    use crate::routing::{Action, Rules};
    let writable = view.rules_path.is_some();
    let total = view.rules.entries.len();
    let mut list = panel()
        .child(
            row()
                .justify_between()
                .child(title("规则顺序"))
                .child(small(format!("{total} 条规则"))),
        )
        .when(total == 0, |card| {
            card.child(
                column()
                    .py(px(24.))
                    .items_center()
                    .gap(px(6.))
                    .child(title("还没有自定义规则"))
                    .child(small("先复制规则文本，再点击导入。")),
            )
        });
    for (index, rule) in view.rules.entries.iter().enumerate() {
        let tint = match rule.action {
            Action::Proxy => PURPLE,
            Action::Direct => GREEN,
            Action::Block => 0xffa0a3,
        };
        list = list.child(
            row()
                .gap(px(8.))
                .min_h(px(43.))
                .px(px(8.))
                .rounded(px(6.))
                .bg(rgb(0x212837))
                .child(
                    div()
                        .w(px(28.))
                        .text_size(px(12.))
                        .text_color(rgb(DIM))
                        .child(format!("{}", index + 1)),
                )
                .child(cell(rule.target.label()))
                .child(
                    div()
                        .w(px(48.))
                        .text_size(px(12.))
                        .text_color(rgb(tint))
                        .child(rule.action.label()),
                )
                .child(
                    button("rule-up", "↑", writable && index > 0)
                        .id(("rule-up", index))
                        .debug_selector(|| format!("rule-up-{index}"))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if writable && index > 0 {
                                view.edit_rules(cx, |rules| {
                                    rules.entries.swap(index, index - 1);
                                    Ok(())
                                });
                            }
                        })),
                )
                .child(
                    button("rule-down", "↓", writable && index + 1 < total)
                        .id(("rule-down", index))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if writable && index + 1 < total {
                                view.edit_rules(cx, |rules| {
                                    rules.entries.swap(index, index + 1);
                                    Ok(())
                                });
                            }
                        })),
                )
                .child(
                    button("rule-remove", "移除", writable)
                        .id(("rule-remove", index))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if writable {
                                view.edit_rules(cx, |rules| {
                                    rules.entries.remove(index);
                                    Ok(())
                                });
                            }
                        })),
                ),
        );
    }
    column()
        .w_full()
        .max_w(px(1140.))
        .mx_auto()
        .p(px(16.))
        .gap(px(16.))
        .child(
            panel()
                .border_color(rgb(0x545482))
                .bg(linear_gradient(
                    125.,
                    linear_color_stop(rgb(0x262941), 0.),
                    linear_color_stop(rgb(0x191f33), 1.),
                ))
                .child(
                    row()
                        .justify_between()
                        .gap(px(8.))
                        .child(title("添加规则"))
                        .child(
                            button("import-rules", "＋ 从剪贴板导入", writable)
                                .bg(rgb(0x5145b3))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if !writable {
                                        return;
                                    }
                                    let copied =
                                        cx.read_from_clipboard().and_then(|item| item.text());
                                    view.edit_rules(cx, |rules| {
                                        let text = copied.as_deref().ok_or_else(|| {
                                            anyhow::anyhow!("剪贴板中没有规则文本")
                                        })?;
                                        for rule in Rules::import(text)? {
                                            if !rules.entries.contains(&rule) {
                                                rules.entries.push(rule);
                                            }
                                        }
                                        Ok(())
                                    });
                                })),
                        ),
                )
                .child(small(
                    "从上到下匹配，首条命中生效。每行填写一个动作和一个匹配项。",
                ))
                .child(
                    row()
                        .gap(px(12.))
                        .px(px(10.))
                        .py(px(8.))
                        .rounded(px(6.))
                        .bg(rgb(0x171d2d))
                        .child(small("direct example.com"))
                        .child(small("proxy 203.0.113.0/24"))
                        .child(small("block full:ads.example.com")),
                )
                .when(!view.rules_feedback.is_empty(), |card| {
                    card.child(
                        small(view.rules_feedback.clone()).text_color(rgb(if view.rules_failed {
                            0xffa0a3
                        } else {
                            GREEN
                        })),
                    )
                }),
        )
        .child(
            panel().child(
                row()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        column()
                            .gap(px(5.))
                            .child(title("未命中规则"))
                            .child(small("默认动作会在下次连接时生效。")),
                    )
                    .child(
                        row()
                            .gap(px(7.))
                            .children(Action::ALL.into_iter().map(|action| {
                                button(action.tag(), action.label(), writable)
                                    .when(view.rules.fallback == action, |control| {
                                        control.bg(rgb(0x5145b3))
                                    })
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        if writable {
                                            view.edit_rules(cx, |rules| {
                                                rules.fallback = action;
                                                Ok(())
                                            });
                                        }
                                    }))
                            })),
                    ),
            ),
        )
        .child(list)
}

fn subscription_page(view: &Dashboard, cx: &mut Context<Dashboard>) -> Div {
    let state = &view.subscriptions;
    let available = !state.busy() && state.writable();
    let can_add = available && state.pending.is_some();
    let can_refresh = available && !state.records.is_empty();
    let pending = state
        .pending
        .as_ref()
        .and_then(|url| url.host_str())
        .map_or_else(
            || "从剪贴板粘贴 HTTP / HTTPS 订阅链接".into(),
            |host| format!("{host} · 链接已粘贴"),
        );
    let mut page = column()
        .w_full()
        .max_w(px(1140.))
        .mx_auto()
        .px(px(22.))
        .py(px(28.))
        .gap(px(22.))
        .child(
            column()
                .py(px(13.))
                .gap(px(9.))
                .child(
                    row()
                        .gap(px(13.))
                        .child(div().text_size(px(30.)).text_color(rgb(PURPLE)).child("◔"))
                        .child(
                            div()
                                .text_size(px(30.))
                                .line_height(px(36.))
                                .font_weight(FontWeight::BOLD)
                                .child("订阅链接"),
                        ),
                )
                .child(small("添加并管理节点订阅，自动获取和更新节点列表").text_size(px(14.))),
        )
        .child(
            panel()
                .p(px(23.))
                .gap(px(15.))
                .border_color(rgb(0x545482))
                .bg(linear_gradient(
                    125.,
                    linear_color_stop(rgb(0x262941), 0.),
                    linear_color_stop(rgb(0x191f33), 1.),
                ))
                .child(title("添加订阅链接"))
                .child(
                    row()
                        .gap(px(10.))
                        .child(
                            row()
                                .flex_1()
                                .h(px(39.))
                                .gap(px(10.))
                                .pl(px(11.))
                                .rounded(px(7.))
                                .border_1()
                                .border_color(rgb(0x536078))
                                .bg(rgb(0x20283a))
                                .child(
                                    small(pending)
                                        .flex_1()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis(),
                                )
                                .child(button("paste-subscription", "粘贴", available).on_click(
                                    cx.listener(move |view, _, _, cx| {
                                        if !available {
                                            return;
                                        }
                                        let result = cx
                                            .read_from_clipboard()
                                            .and_then(|item| item.text())
                                            .ok_or_else(|| anyhow::anyhow!("剪贴板中没有链接"))
                                            .and_then(|text| crate::subscriptions::validate(&text));
                                        match result {
                                            Ok(url) => {
                                                view.subscriptions.pending = Some(url);
                                                view.subscriptions.feedback.clear();
                                                view.subscriptions.failed = false;
                                            }
                                            Err(error) => {
                                                view.subscriptions.pending = None;
                                                view.subscriptions.error(error.to_string());
                                            }
                                        }
                                        cx.notify();
                                    }),
                                )),
                        )
                        .child(
                            button("add-subscription", "＋ 添加订阅", can_add)
                                .h(px(39.))
                                .bg(rgb(if can_add { 0x6654df } else { 0x353052 }))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if can_add && let Some(url) = &view.subscriptions.pending {
                                        let url = url.to_string();
                                        view.subscriptions
                                            .start(vec![url], view.subscription_proxy());
                                        cx.notify();
                                    }
                                })),
                        ),
                )
                .when(!state.feedback.is_empty(), |panel| {
                    panel.child(
                        small(state.feedback.clone()).text_color(rgb(if state.failed {
                            0xffa0a3
                        } else {
                            MUTED
                        })),
                    )
                }),
        )
        .child(
            row()
                .justify_between()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(19.))
                        .child(format!("我的订阅（{}）", state.records.len())),
                )
                .child(
                    button("refresh-subscriptions", "↻ 全部更新", can_refresh).on_click(
                        cx.listener(move |view, _, _, cx| {
                            if can_refresh {
                                let urls = view
                                    .subscriptions
                                    .records
                                    .iter()
                                    .map(|record| record.url.clone())
                                    .collect();
                                view.subscriptions.start(urls, view.subscription_proxy());
                                cx.notify();
                            }
                        }),
                    ),
                ),
        );
    if state.records.is_empty() {
        return page.child(
            panel()
                .min_h(px(108.))
                .justify_center()
                .items_center()
                .child(title("还没有订阅"))
                .child(small("复制提供方的订阅链接，粘贴后点击添加。")),
        );
    }
    for (index, record) in state.records.iter().enumerate() {
        let minutes = crate::subscriptions::now().saturating_sub(record.updated) / 60;
        let updated = match minutes {
            0 => "刚刚更新".into(),
            1..=59 => format!("{minutes} 分钟前更新"),
            60..=1439 => format!("{} 小时前更新", minutes / 60),
            _ => format!("{} 天前更新", minutes / 1440),
        };
        let summary = if record.skipped == 0 {
            updated
        } else {
            format!("{updated} · 跳过 {} 项", record.skipped)
        };
        page = page.child(
            panel().p(px(16.)).min_h(px(74.)).justify_center().child(
                row()
                    .gap(px(12.))
                    .child(
                        row()
                            .size(px(36.))
                            .justify_center()
                            .flex_shrink_0()
                            .rounded(px(8.))
                            .bg(rgb(0x332b58))
                            .text_color(rgb(PURPLE))
                            .child("↗"),
                    )
                    .child(
                        column()
                            .flex_1()
                            .gap(px(5.))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .whitespace_nowrap()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(record.name()),
                            )
                            .child(
                                small(summary)
                                    .whitespace_nowrap()
                                    .overflow_hidden()
                                    .text_ellipsis(),
                            ),
                    )
                    .child(
                        row()
                            .gap(px(7.))
                            .flex_shrink_0()
                            .child(dot(GREEN))
                            .child(small(format!("{} 个节点", record.links.len()))),
                    )
                    .child(
                        button("refresh-feed", "↻ 更新", available)
                            .id(("refresh-feed", index))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if available
                                    && let Some(record) = view.subscriptions.records.get(index)
                                {
                                    view.subscriptions
                                        .start(vec![record.url.clone()], view.subscription_proxy());
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        button("remove-feed", "移除", available)
                            .id(("remove-feed", index))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if available {
                                    if view.subscriptions.remove(index) {
                                        view.rebuild_nodes(None);
                                    }
                                    cx.notify();
                                }
                            })),
                    ),
            ),
        );
    }
    page
}

impl Dashboard {
    fn ready(&self) -> bool {
        matches!(self.status, "本地代理已就绪" | "TUN 已启动")
    }
}

impl Render for Dashboard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let narrow = window.viewport_size().width < px(1090.);
        let content = match self.page {
            Page::Overview => {
                let primary = column()
                    .debug_selector(|| "overview-main".into())
                    .flex_1()
                    .gap(px(13.))
                    .child(
                        div()
                            .flex()
                            .gap(px(11.))
                            .child(connection(self, cx))
                            .child(metrics()),
                    )
                    .child(traffic(window.viewport_size().height < px(730.)))
                    .child(nodes(self, cx));
                let secondary = column()
                    .debug_selector(|| "overview-side".into())
                    .gap(px(13.))
                    .when(!narrow, |side| side.w(px(279.)).flex_shrink_0())
                    .child(controls(self, cx))
                    .child(logs(self, false, cx));
                div()
                    .flex()
                    .gap(px(13.))
                    .when(narrow, |layout| layout.flex_col())
                    .child(primary)
                    .child(secondary)
            }
            Page::Nodes => column().gap(px(13.)).child(nodes(self, cx)),
            Page::Subscriptions => subscription_page(self, cx),
            Page::Routing => routing_page(self, cx),
            Page::Logs => column().child(logs(self, true, cx)),
            Page::Settings => settings_page(self, cx),
        };
        div()
            .flex()
            .id("dashboard")
            .size_full()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .track_focus(&self.focus)
            .tab_group()
            .tab_stop(false)
            .on_key_down(|event, window, cx| {
                if event.keystroke.key == "tab" {
                    if event.keystroke.modifiers.shift {
                        window.focus_prev();
                    } else {
                        window.focus_next();
                    }
                    cx.stop_propagation();
                }
            })
            .child(sidebar(self, narrow, cx))
            .child(
                column().flex_1().min_h_0().child(header(self.page)).child(
                    div()
                        .id("content")
                        .flex_1()
                        .overflow_y_scroll()
                        .p(px(16.))
                        .child(content),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        ClipboardItem, KeyUpEvent, Keystroke, Modifiers, TestAppContext, VisualTestContext, size,
    };

    fn click(cx: &mut VisualTestContext, selector: &'static str) {
        let bounds = cx.debug_bounds(selector).expect(selector);
        cx.simulate_click(bounds.center(), Modifiers::none());
    }

    #[gpui::test]
    fn settings_page_saves_preferences_and_restores_defaults(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let (view, cx) = cx.add_window_view(|window, cx| {
            let mut dashboard = Dashboard::new(window, cx);
            dashboard.settings_path = Some(path.clone());
            dashboard
        });
        click(cx, "设置");
        click(cx, "settings-tun");
        click(cx, "settings-refresh");
        click(cx, "settings-access");
        let saved = crate::settings::load(&path).unwrap();
        assert_eq!(saved.default_mode, Mode::Tun);
        assert!(!saved.auto_refresh && !saved.access_logs);
        cx.update(|_, cx| assert_eq!(view.read(cx).mode, Mode::Tun));
        click(cx, "reset-settings");
        assert_eq!(
            crate::settings::load(&path).unwrap(),
            crate::settings::Settings::default()
        );
        cx.update(|_, cx| assert_eq!(view.read(cx).mode, Mode::Local));
    }

    #[gpui::test]
    fn logs_filter_copy_and_clear(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|window, cx| {
            use crate::logs::{Level, Source};
            let mut view = Dashboard::new(window, cx);
            view.record("客户端连接失败", "test problem".into(), true);
            view.push_log(
                Source::Xray,
                Level::Traffic,
                "",
                "accepted tcp:example.test:443".into(),
            );
            view
        });
        click(cx, "连接日志");
        click(cx, "访问");
        click(cx, "copy-logs");
        let clipboard = cx.read_from_clipboard().unwrap().text().unwrap();
        assert!(clipboard.contains("[Xray] [访问] accepted tcp:example.test:443"));
        assert!(!clipboard.contains("test problem"));
        click(cx, "警告与错误");
        click(cx, "copy-logs");
        let clipboard = cx.read_from_clipboard().unwrap().text().unwrap();
        assert!(clipboard.contains("客户端连接失败 — test problem"));
        assert!(!clipboard.contains("example.test"));
        click(cx, "clear-logs");
        cx.update(|_, cx| assert!(view.read(cx).logs.is_empty()));
    }

    #[gpui::test]
    fn routing_page_reorders_and_persists_rules(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("routing.json");
        let (view, cx) = cx.add_window_view(|window, cx| {
            let mut view = Dashboard::new(window, cx);
            view.rules_path = Some(path.clone());
            view
        });
        click(cx, "路由规则");
        cx.write_to_clipboard(ClipboardItem::new_string(
            "direct example.com\nblock full:ads.example.com".into(),
        ));
        click(cx, "import-rules");
        let persisted = crate::routing::load(&path).unwrap();
        assert_eq!(persisted.entries.len(), 2);
        assert_eq!(persisted.entries[0].target.label(), "example.com");
        click(cx, "rule-up-1");
        let persisted = crate::routing::load(&path).unwrap();
        assert_eq!(persisted.entries[0].target.label(), "full:ads.example.com");
        click(cx, "direct");
        assert_eq!(
            crate::routing::load(&path).unwrap().fallback,
            crate::routing::Action::Direct
        );
        cx.write_to_clipboard(ClipboardItem::new_string(
            "proxy example.net\nunknown invalid".into(),
        ));
        click(cx, "import-rules");
        cx.update(|_, cx| assert!(view.read(cx).rules_failed));
        assert_eq!(crate::routing::load(&path).unwrap().entries.len(), 2);
    }

    #[gpui::test]
    fn subscription_page_adds_nodes_and_removes_them(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let link = "vless://6cece960-04ec-4655-800e-bb0a28c290d1@127.0.0.1:443";
        let url = crate::subscriptions::tests::serve(vec![(200, link.into())]);
        let (view, cx) = cx.add_window_view(|window, cx| {
            let mut view = Dashboard::new(window, cx);
            view.subscriptions =
                crate::subscriptions::Manager::new(Ok(directory.path().join("feeds.json")));
            view
        });
        click(cx, "订阅");
        cx.write_to_clipboard(ClipboardItem::new_string(url));
        click(cx, "paste-subscription");
        click(cx, "add-subscription");
        cx.update(|_, cx| assert!(view.read(cx).subscriptions.busy()));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let busy = cx.update(|_, cx| {
                view.update(cx, |view, cx| {
                    view.poll(cx);
                    view.subscriptions.busy()
                })
            });
            if !busy {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert_eq!(view.read(cx).subscriptions.records.len(), 1);
            assert_eq!(view.read(cx).nodes.len(), 1);
        });
        click(cx, "remove-feed");
        cx.update(|_, cx| {
            assert!(view.read(cx).subscriptions.records.is_empty());
            assert!(view.read(cx).nodes.is_empty());
        });
    }

    #[gpui::test]
    fn header_stays_horizontal_at_the_reported_window_size(cx: &mut TestAppContext) {
        let (_, cx) = cx.add_window_view(Dashboard::new);
        for (width, height) in [(1232., 632.), (820., 540.)] {
            cx.simulate_resize(size(px(width), px(height)));
            cx.run_until_parked();
            let copy = cx.debug_bounds("header-copy").unwrap();
            let config = cx.debug_bounds("header-config").unwrap();
            assert!(
                copy.size.width >= px(230.),
                "title column was squeezed: {copy:?}"
            );
            assert!(copy.right() <= config.left());
            assert!(config.right() <= px(width));
            assert!(copy.size.height <= px(70.));
        }
    }

    #[gpui::test]
    fn dashboard_keeps_columns_visible_when_resized(cx: &mut TestAppContext) {
        let (_, cx) = cx.add_window_view(Dashboard::new);
        cx.simulate_resize(size(px(1350.), px(880.)));
        cx.run_until_parked();
        let wide_main = cx.debug_bounds("overview-main").unwrap();
        let wide_side = cx.debug_bounds("overview-side").unwrap();
        assert!(wide_main.right() < wide_side.left());
        assert_eq!(wide_main.top(), wide_side.top());
        assert!(wide_side.right() <= px(1350.));

        cx.simulate_resize(size(px(820.), px(540.)));
        cx.run_until_parked();
        let compact_main = cx.debug_bounds("overview-main").unwrap();
        let compact_side = cx.debug_bounds("overview-side").unwrap();
        assert_eq!(compact_main.left(), compact_side.left());
        assert!(compact_main.bottom() < compact_side.top());
        assert!(compact_side.right() <= px(820.));
    }

    #[gpui::test]
    fn keyboard_navigation_works_from_a_fresh_window(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(Dashboard::new);
        cx.simulate_keystrokes("tab");
        cx.update(|window, cx| {
            assert!(
                window.focused(cx).is_some(),
                "Tab did not enter the navigation"
            )
        });
        cx.simulate_keystrokes("tab");
        cx.simulate_event(KeyUpEvent {
            keystroke: Keystroke::parse("enter").unwrap(),
        });
        cx.update(|_, cx| assert!(view.read(cx).page == Page::Nodes));
    }

    #[gpui::test]
    fn clipboard_import_keeps_nodes_and_logs_available(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(Dashboard::new);
        click(cx, "节点");
        cx.write_to_clipboard(ClipboardItem::new_string("not a node".into()));
        click(cx, "import");
        cx.update(|_, cx| assert!(view.read(cx).failed));
        for address in ["127.0.0.1:443", "127.0.0.1:8443", "127.0.0.1:443"] {
            cx.write_to_clipboard(ClipboardItem::new_string(format!(
                "vless://6cece960-04ec-4655-800e-bb0a28c290d1@{address}"
            )));
            click(cx, "import");
        }
        cx.update(|_, cx| {
            let dashboard = view.read(cx);
            assert_eq!(dashboard.nodes.len(), 2);
            assert_eq!(dashboard.selected, 0);
            assert!(!dashboard.failed);
        });
        click(cx, "node-1");
        cx.update(|_, cx| assert_eq!(view.read(cx).selected, 1));
        click(cx, "连接日志");
        cx.update(|_, cx| {
            let dashboard = view.read(cx);
            assert!(dashboard.page == Page::Logs);
            assert_eq!(dashboard.logs.len(), 4);
        });
        click(cx, "概览");
        click(cx, "tun");
        cx.update(|_, cx| assert!(view.read(cx).mode == Mode::Tun));
        click(cx, "tun");
        cx.update(|_, cx| assert!(view.read(cx).mode == Mode::Local));
    }
}
