# AGENTS.md

Windows 托盘工具，读取 MCHOSE / 迈从鼠标电量。纯 Rust，无 WebView。Windows 专属（hidapi `windows-native`，`windows_subsystem = "windows"`）。

## 常用命令

```powershell
cargo run -- --scan --debug   # 仅 HID 扫描 + 协议探测，不启动托盘；新增/调试型号时先跑这个
cargo run                      # 启动托盘
cargo build --release          # 打包（release 已开 lto + strip + panic=abort）
```

注意：`--scan` 在 release 下因 `windows_subsystem = "windows"` 不会显示控制台输出；需要看 `--scan` 结果时用 debug 构建，或临时加 console feature / 改 cfg_attr。`--debug` 当前未被 `main.rs` 解析，仅 `--scan` 实际生效。

## 架构要点

- `main.rs`：解析 `--scan` 分支；否则进入 winit `EventLoop`，启动 60 秒刷新线程 + 托盘控制器。
- `protocols/mod.rs`：`BatteryProtocol` trait + `try_read_with_any_protocol`，按顺序尝试各协议。**新增型号在这里注册 `all_protocols()`**。
- `protocols/a7_compatible.rs`：A7 已实现，按 VID `0x5253` / PID `0x1021` 匹配；使用 feature report（report id `0x11`，长度 64），收发字节均做 `^ 0xff` 异或。
- `protocols/g3.rs`：G3 A 已按 MCHOSE `/gmouse` 网页源码实现，匹配 `a8a5:2255` + usage page `0xff01` / usage `0x10`；通过 `write` 发送 report id `0x00`、payload `55 30 a5 0b 2e 01 01 01 00 00...`，从 input report `aa 30` 解析 `response[8]` 电量、`response[9]` 充电标志。
- `hid_scan.rs`：`looks_like_mchose` 用厂商/产品名 + VID `0x5253` 匹配；`stable_id_from_path` 是 FNV-1a，菜单 item id 依赖它，不要随意改。
- `tray.rs` / `app_state.rs` / `icons.rs`：托盘 UI、状态、电量图标生成。

## 协议移植约定

A7 协议来自公开项目 `MaiCong-A7-Check-Battery`，逻辑用 Rust 重写而非照搬字节。新协议应保持相同风格：在 `protocols/` 下独立模块、实现 `BatteryProtocol`、`matches()` 用 VID/PID 精确匹配优先，name 字符串保持简短（写入托盘菜单）。

## 文档

`docs/` 下有协议说明与 WebHID 抓包脚本（中文文件名，编码非 UTF-8 时可能乱码，按内容理解即可）。改协议前优先参考。
