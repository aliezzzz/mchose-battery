# MCHOSE 网页设备与协议对照

来源：`https://www.mchose.com.cn` 主 HUB + `/gmouse` 子应用（2026 抓取）。

## 官方支持列表（HUB 文案）

鼠标：K7 Ultra、A7 系列、M7 系列、L7 系列、G7 系列、G3 A、G3 Ultra、G3 V2、G3 V2 Pro、A5 Ultra、A5 V2 Ultra、A5 V3、AX5 V2。

## 协议族

| 族 | 模块 | 典型型号 | 识别要点 | 电量命令 |
|----|------|----------|----------|----------|
| A 旧 A5/AX5 | `a5_pro` | A5 Pro / Pro Max / AX5 Pro（老驱动） | VID `0x2023` MI02 | feature `[6]=0x83` |
| B 旧 A7 | `a7_compatible` | A7（公开项目） | `0x5253:0x1021` | feature `0x11` + XOR |
| C gmouse | `g3` / Gmouse | G3 A/V2、A7e、G7 | usage `0xFF01`，VID `0xA8A5`/`0xA8A4`/`0x3837` | write `55 30 a5…` → `aa 30` |
| D 主 HUB | `hub_native` | K7/M7/L7、A5 Ultra/V2/V3、AX5 V2、G3 Ultra、A7 V2+ | VID `0x3837` + 厂商 usage page | 有线 `06 87…` / 无线 `13 4a…`；上报 cmd=10 type=5 |

## gmouse 有线 VID/PID 示例

| 型号 | VID:PID (dec) | hex |
|------|---------------|-----|
| G3 V2 有线 | 14391:16780 | `3837:418c` |
| G3 V2 Pro 有线 | 14391:16450 | `3837:4042` |
| A7e 有线 | 14391:16402 | `3837:4012` |
| A7e Pro 有线 | 14391:16403 | `3837:4013` |
| G3 A 无线 | — | `a8a5:2255` |

## 说明

- 旧 A5 Pro **不在**新网页支持列表；继续用协议 A。
- 主 HUB JS 强混淆，PID 表含大量键盘；实现以 usage page + 探测成功为准。
- 无对应真机时协议 D 为 best-effort，需 `--scan` 回传日志再收紧。
