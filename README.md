# Xray Connect

用 GPUI 编写的桌面代理客户端。可从剪贴板导入 VLESS TCP 节点，在当前会话中保留和切换多个节点。支持无 TLS、TLS 和 REALITY，以及 `xtls-rprx-vision`。提供本地 SOCKS5 和 TUN 两种模式，并显示当前会话的连接事件。

## 运行

安装 rustup。项目通过 `rust-toolchain.toml` 使用 Rust 1.100.0。Windows 首选 MSVC 工具链和 Visual Studio C++ Build Tools。macOS 需要 Xcode Command Line Tools。

准备与系统架构一致的 [Xray-core v26.11.9](https://github.com/XTLS/Xray-core/releases/tag/v26.11.9) 和 [sing-box v1.14.4](https://github.com/SagerNet/sing-box/releases/tag/v1.14.4)。TUN 配置使用 sing-box 1.14 的原生 DNS 配置能力。可将二者放在客户端可执行文件旁，或通过 `GPUI_PROXY_XRAY` 和 `GPUI_PROXY_SING_BOX` 指定可信的绝对路径。普通权限下也可通过 PATH 查找。

```sh
cargo build --release
```

- **Windows** 在管理员终端里运行 `target/release/gpui-proxy.exe` 以使用 TUN。保留终端，客户端通过控制台信号请求核心正常退出。
- **macOS** 从当前桌面会话的终端运行 `sudo ./target/release/gpui-proxy` 以尝试 TUN 原型。这里使用 sing-box 的 utun 入口，签名分发和 Network Extension 尚待实现。
- **本地 SOCKS5** 使用普通权限启动，连接后将需要代理的应用指向窗口显示的 SOCKS5 地址。

复制 `vless://` 链接，点击「从剪贴板导入」，选择代理模式，再点击「连接」。窗口关闭时会请求断开。窗口的就绪状态表示本地组件已启动，远端可达性需要实际访问确认。

## 订阅

在「订阅」页点击「粘贴」，然后点击「添加订阅」。支持 HTTP / HTTPS 上的明文链接列表、Base64 链接列表和包含 `proxies` 的 Clash YAML。客户端导入其中受支持的 VLESS TCP 节点，统计跳过的条目，并将订阅及最近一次成功的节点快照保存在当前用户的本地数据目录。

订阅支持单独更新、全部更新和移除。客户端运行时会检查超过 24 小时未更新的订阅。更新失败会保留上一次的节点快照。使用本地 SOCKS5 连接时，订阅请求通过该本地代理发送。

## 路由规则

在「路由规则」页从剪贴板导入规则。每行一个动作和一个匹配项，例如：

```text
block full:ads.example.com
direct example.com
proxy 203.0.113.0/24
```

普通域名匹配自身和子域名。`full:` 只匹配完整域名。IP 与 CIDR 匹配流量中的目的 IP。规则按页面顺序检查，首条命中生效，可上移、下移或移除。未命中的流量使用页面选定的默认动作，新设置在下次连接时生效。

本地 SOCKS5 模式由 Xray 执行规则，TUN 模式由 sing-box 执行。TUN 的 DNS 接管仍使用现有的代理上游，节点服务器自身的连接继续使用原来的物理网络。

## 连接日志

概览显示最近记录。「连接日志」页实时显示客户端事件、Xray 访问记录和 sing-box 核心输出，可按访问、核心、警告与错误筛选，也可复制筛选结果或清空。时间为本次客户端会话内的经过时间。记录仅在内存中保留，最多 240 条，关闭客户端后清除。

## 设置

「设置」页可保存默认接入方式、自定义 Xray 与 sing-box 路径、订阅自动刷新和访问日志选项。核心路径和访问日志设置在重新连接后生效。Windows 与 macOS 的开机启动仅在当前用户登录后打开客户端，连接仍由用户发起。macOS 请以普通用户身份配置登录启动。

## TUN 验收

TUN 将 TCP 和 UDP 交给本地 Xray SOCKS 入站。客户端在接管路由前解析节点地址，并把实际使用的地址排除在 TUN 路由之外。经 TUN 的 53 端口流量由 sing-box 转发至通过 Xray 访问的 DNS 上游。

在两端分别使用真实节点测试忽略系统代理的应用、TCP、UDP 和 DNS。测试前后对比路由及 DNS 设置，再检查断开、核心异常退出与切换网络后的行为。本地 SOCKS5 模式目前需要在使用方配置代理。自动修改系统代理、节点持久化、托盘和安装包尚未实现。
