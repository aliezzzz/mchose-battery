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
- `protocols/a5_pro.rs`：旧 A5/AX5 Pro（官方已弃更），VID `0x2023` + 已知 PID + **interface 2**；feature report 长度 65。电量：`[4]=2,[6]=0x83` → `[8]` 电量、`[7]` 充电。DPI：`0x81` 档位表 + `0x82` 当前档。来自旧驱动 `DriverCore.exe`。
- `protocols/a7_compatible.rs`：旧 A7，VID `0x5253` / PID `0x1021`；feature report id `0x11`，收发 `^ 0xff`。
- `protocols/g3.rs`（`GmouseProtocol`）：`/gmouse` 网页族（G3 A/V2、A7e、G7…）。匹配 usage page `0xff01` + VID `0xa8a5`/`0xa8a4`/`0x3837`；write `55 30 a5 0b 2e…`，input `aa 30` 解析电量/充电；另可读 DPI 配置。
- `protocols/hub_native.rs`：主 HUB 新型号（K7/M7/L7、A5 Ultra/V2/V3、AX5 V2、G3 Ultra、A7 V2+…）。VID 主 `0x3837`；有线 feature `06 87…`、无线 `13 4a…`；解析 command=10/type=5 或 Bbe 布局。无真机时仅按网页命令落地，需用户 `--scan` 校验。
- `protocols/mc_new_mouse.rs`：新一代 Nordic 鼠标（K5 Pro/Ultra/典藏版、K5 V2、A5 V3、A7 V3、K7 V2…）。K5 Ultra 典藏版 2.4G PID `0x1018`，有线 `0x4038`。output report id `0x4D`，64 字节报文，cmd `0x0900`；body[11] 充电、body[12] 电量。DPI：cmd `0x0002` 读配置档，再 `0x0003`（data `[profile, 0]`）读档位表，body[2] 档数、body[3] 当前档、body[5..] 每档 uint16 LE。来自 HUB `newMouse:cmdStruct`。
- `hid_scan.rs`：`looks_like_mchose` 含名称 + VID `0x5253`/`0x2023`/`0x3837`/`0xa8a5`/`0xa8a4`。
- `tray.rs` / `app_state.rs` / `icons.rs`：托盘 UI、状态、电量图标生成。

## 协议移植约定

A7 协议来自公开项目 `MaiCong-A7-Check-Battery`，逻辑用 Rust 重写而非照搬字节。新协议应保持相同风格：在 `protocols/` 下独立模块、实现 `BatteryProtocol`、`matches()` 用 VID/PID 精确匹配优先，name 字符串保持简短（写入托盘菜单）。

## 文档

`docs/` 下有协议说明与 WebHID 抓包脚本（中文文件名，编码非 UTF-8 时可能乱码，按内容理解即可）。改协议前优先参考。
