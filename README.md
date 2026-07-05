# Video Player

一个 Windows 桌面视频播放器，基于 **libmpv** 内核 + **Tauri 2** + **Vue 3**。专注于流畅播放、自定义标签系统与炫酷的毛玻璃界面。

> 状态：个人项目，自用为主。功能完整但仍在打磨中。

## ✨ 功能

- **播放核心**（由 libmpv 提供顶级画质）
  - 硬件解码（D3D11 / `vo=gpu-next`）
  - 倍速播放（0.5x–4x，保留音调）
  - 多音轨切换、外挂字幕（srt/ass/vtt）、字幕轨切换
  - 自动加载同目录同名字幕（`sub-auto=fuzzy`）
  - 截图（含/不含字幕，保存到"图片/Screenshots"）
  - A-B 循环、逐帧步进
  - 画面比例切换、翻转、色彩调节

- **标签系统** ⭐
  - 内容指纹（文件大小 + 头尾 64MB SHA256）绑定，**文件改名/移动标签不丢**
  - 预置标签：星级（1–5★）、画质（480p/720p/1080p/4K）
  - 自定义标签类型：枚举型（下拉）/自由型（文本）
  - 双击画面（T 键 / 右键）唤出磨砂玻璃标签卡片

- **记忆播放进度**
  - 自动恢复到上次播放位置（基于内容 hash）

- **标签搜索**
  - `Ctrl+F` 唤出磨砂玻璃搜索浮层
  - 实时筛选（按文件名或任意标签值）
  - 单击播放 / 右键"在文件夹中显示"

- **炫酷界面**
  - 透明 WebView 叠在 mpv 视频之上的单窗口架构
  - 毛玻璃面板、弹性动画、自动隐藏控制栏
  - 拖拽文件打开（视频直接播，字幕自动找同名视频）

## 🛠 技术栈

| 层 | 技术 |
|---|---|
| 播放内核 | [libmpv](https://github.com/mpv-player/mpv) (LGPL) |
| mpv 桥接 | [tauri-plugin-libmpv](https://github.com/nini22P/tauri-plugin-libmpv) (wid 窗口嵌入) |
| 桌面框架 | [Tauri 2](https://v2.tauri.app/) |
| 后端 | Rust（rusqlite + sha2） |
| 前端 | Vue 3 + TypeScript + Vite |
| 存储 | SQLite（标签 + 播放进度） |

## 🚀 构建

### 前置要求

- [Node.js](https://nodejs.org/) ≥ 18
- [Rust](https://www.rust-lang.org/tools/install)（stable, MSVC 目标）
- [Visual Studio Build Tools 2022](https://visualstudio.microsoft.com/visual-cpp-build-tools/)（"使用 C++ 的桌面开发"组件）
- Windows 11（自带 WebView2）

### 下载 libmpv 依赖

仓库不包含 libmpv 二进制（体积大 + LGPL）。请手动下载并放到 `src-tauri/lib/`：

1. 从 [tauri-plugin-libmpv releases](https://github.com/nini22P/tauri-plugin-libmpv/releases) 下载 `libmpv-wrapper-windows-x86_64.zip`，解压出 `libmpv-wrapper.dll`
2. 从 [zhongfly/mpv-winbuild releases](https://github.com/zhongfly/mpv-winbuild/releases) 下载 `mpv-dev-lgpl-x86_64-*.7z`，解压出 `libmpv-2.dll`（**注意用 lgpl 版，不要 v3**）
3. 把两个 DLL 放到 `src-tauri/lib/` 目录：

```
src-tauri/lib/
├── libmpv-wrapper.dll
└── libmpv-2.dll
```

### 开发运行

```bash
npm install
npm run tauri dev
```

### 打包

```bash
npm run tauri build
```

产物在 `src-tauri/target/release/`。

## ⌨️ 快捷键

| 键 | 功能 |
|---|---|
| `Space` | 播放/暂停 |
| `←` / `→` | 后退/前进 10 秒 |
| `↑` / `↓` | 音量 +5/-5 |
| `M` | 静音 |
| `F` | 全屏 |
| `T` | 标签卡片 |
| `Ctrl+F` | 搜索 |
| `S` | 截图 |
| `C` | 切换字幕 |
| `,` / `.` | 逐帧后退/前进 |
| 双击 | 全屏 |

## 📄 许可证

[MIT](./LICENSE)

本项目使用 libmpv（LGPL-2.1+）作为动态链接库，用户需自行下载。
