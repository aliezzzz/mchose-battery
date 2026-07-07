# MCHOSE Battery Tray

一个无窗口、无 WebView 的 Windows 托盘小工具，用于读取 MCHOSE / 迈从鼠标电量。

当前目标：

- 纯 Rust 常驻托盘程序
- 右键菜单显示当前设备、电量、充电状态
- 左键切换当前鼠标或刷新
- 优先支持 A7 与 G3 A
- 后续通过协议适配器扩展更多型号

## 快速运行

```powershell
cargo run -- --scan --debug
```

这个命令只做 HID 扫描和协议探测，不启动托盘。建议先用它验证 A7 / G3 的返回结果。

启动托盘：

```powershell
cargo run
```

打包 release：

```powershell
cargo build --release
```

## 当前状态

这是一个可继续开发的项目骨架。A7 协议已按公开项目 `MaiCong-A7-Check-Battery` 的逻辑移植；G3 A 已按 MCHOSE `/gmouse` 网页源码实现，并在真机 `MCHOSE G3 A`（`a8a5:2255`）上通过 `--scan --debug` 读到电量。
