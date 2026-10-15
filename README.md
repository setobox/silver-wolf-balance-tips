<p align="center">
  <img src="docs/assets/silverwolf-ribbon.png" alt="银狼紫蓝缎带项目图标" width="160" height="160" />
</p>

# Silverwolf Balance Tips

Windows 上的轻量 Codex 额度挂件。Rust 查询额度、处理窗口跟随，Tauri + Vue 显示角色和气泡。pnpm monorepo 为后续 DSH / DeepSeek 插件预留包边界。

## 已实现的源码功能

- 使用用户提供的银狼人物，透明 PNG 素材；点击展开白底粗描边思考气泡，再次点击保持显示并重新计时，工具栏按钮可手动收起。
- ChatGPT 登录账户的 Codex 配额：剩余百分比、窗口时长、重置倒计时。
- 支持接口返回的不同窗口和额度桶，不把窗口固定为 5 小时 / 每周。
- 可见时每 60 秒刷新，可手动刷新；旧快照、未登录、API Key 登录、超时有明确提示。
- 拖动角色移动，滚轮或气泡内按钮缩放（60%–160%）。
- Windows 窗口跟随：位置变化事件触发更新，密集通知按 16ms 合并并读取最新坐标；静止时每秒兜底检查，前台切换、最小化、恢复及拖动结束即时响应。隐藏时跳过定位计算，恢复时先定位再显示；设置和挂件拖动结束会主动唤醒跟随线程。
- 跟随模式不设为全局置顶；通过原生窗口区域裁剪，角色 / 气泡以外的区域不阻挡点击。
- 托盘菜单可切换跟随 / 桌面模式、重置位置大小、退出；单实例。
- 本地保存设置和位置，不持久化额度、账户身份或凭据。
- 设置使用紧凑浮层；额度提示默认 1 秒后收起，可选 3 秒、常驻或自定义 1–3600 秒。鼠标停留、键盘操作及设置打开期间暂停计时。
- 刷新时保留已有额度，仅在底部显示「更新中」；成功后替换数据，失败或切换连接时清除旧数据。组件规范见 [设计规范](docs/component-guidelines.md)。

当前阶段：Windows 离线编译、Rust / Node 测试、前端类型检查和 Clippy 已运行。挂件已在本机读取真实额度，点击和缩放已实测；用户手动检查拖动和宿主窗口跟随反馈未见异常。混合 DPI 与多显示器等场景尚未全部验收，详见验收记录。

## 手动准备环境

本仓库不自动安装工具链。需要由你自行安装：

先运行 `node scripts/check-environment.mjs` 可离线检查当前目录的依赖与工具链。该脚本只读取本地文件和 Visual Studio 安装信息，不会安装或下载。项目显式设置 `verifyDepsBeforeRun: error`，禁止 pnpm 11 在运行脚本时自动补装依赖；缺少依赖时必须由你手动执行 `pnpm install`。

1. Node.js 24.14+、pnpm 11.21.0。
2. Rust stable，目标 `x86_64-pc-windows-msvc`，建议包含 rustfmt 和 clippy。
3. Visual Studio 2022 Build Tools 的“使用 C++ 的桌面开发”：MSVC v143 和 Windows 10/11 SDK。
4. Microsoft Edge WebView2 Runtime（很多 Windows 系统已安装）。
5. Codex 桌面端或 CLI，并使用 ChatGPT 账户登录。API Key 登录不提供这里的订阅额度。

在根目录手动执行（这些命令会下载依赖）：

```powershell
pnpm install
cargo fetch
```

依赖锁文件 `pnpm-lock.yaml` 和 `Cargo.lock` 已生成，应纳入版本管理。后续使用 `pnpm install --frozen-lockfile` 和 `cargo ... --locked`。

## 开发与构建

```powershell
pnpm dev         # Windows 桌面应用，自动启动 Vite
pnpm dev:web     # 仅浏览器界面，不读取账户，不提供原生窗口行为
pnpm build:web   # 前端类型检查与打包
pnpm build       # Rust release + NSIS 安装包
```

首次 `pnpm build` 可能还会下载 Tauri 的 NSIS 打包工具。安装器默认可能联网安装缺失的 WebView2。请在允许联网下载时自行运行。只编译程序可在桌面包目录执行 `pnpm exec tauri build --no-bundle`，但 Cargo 仍需依赖已缓存。

产物位于根目录 `target/release/`，安装包在 `target/release/bundle/nsis/`。

依赖缓存齐全时，可以在根目录离线构建独立 EXE，跳过安装包工具：

```powershell
pnpm build:web
cargo build --release -p silverwolf-desktop --features tauri/custom-protocol --offline --locked
```

产物为 `target/release/silverwolf-desktop.exe`，前端资源内嵌，无需另开 Vite。此命令不会下载缺失的 Cargo 依赖；缺少缓存时会报错。

默认跟随 Codex：若没有打开 Codex 窗口，角色会隐藏，托盘图标仍在。托盘选择“显示在桌面”即可找到它；重复启动应用也会临时显示在桌面。默认不添加开机自启、自动更新或外部守护服务。

## Codex CLI 发现

按以下顺序查找：

1. 气泡“连接设置”中的绝对路径，或 `CODEX_EXECUTABLE` 环境变量。
2. PATH 内的原生 `codex.exe`。
3. `%LOCALAPPDATA%/OpenAI/Codex/bin/*/codex.exe`。
4. 可读取的 WindowsApps 中 Codex 应用的内置 CLI。
5. 常见 npm Codex 包中的 Windows x64 原生可执行文件。

没有执行 `.cmd` / PowerShell 包装脚本；若使用非标准、ARM64 或其他布局，在连接设置中指定原生 CLI 的完整路径即可。指定路径不会通过 shell 拼接执行。

Rust 启动 `codex app-server`，通过 stdio JSONL 完成 `initialize` / `initialized` 后，仅请求 `account/read` 和 `account/rateLimits/read`。不创建聊天、不调用模型、不直接读取 `auth.json`。返回错误会脱敏，查询限时 20 秒，完成后回收子进程。

设置位于 Tauri 的用户配置目录（Windows 通常为 `%APPDATA%/dev.silverwolf.balance/settings.json`）。角色素材、JS 与字体均来自本地，无远程资源。

## 包结构

```text
apps/desktop/                 Tauri 桌面应用 + Vue UI
  src/components/             角色、气泡、桌面交互组合
  src/composables/             额度刷新生命周期
  src-tauri/src/windows.rs     Win32 发现、跟随、层级、点击区域
  src-tauri/src/settings.rs    配置验证与原子保存
crates/codex-usage/            独立于 Tauri 的 Rust Codex 查询实现
packages/core/                无 UI / 宿主依赖的数据契约与格式化函数
scripts/generate-icons.mjs     从缎带原图导出应用与托盘图标
```

`pnpm-workspace.yaml` 包含 `apps/*` 与 `packages/*`，根 Cargo workspace 管理 Rust crate。当前不引入 Turborepo、不做 MCP 注册，也不创建空的 DeepSeek 实现。

项目图标参考银狼头上的紫蓝缎带，由内置 imagegen 生成；README、应用和托盘使用同一图案。原图为 `docs/assets/silverwolf-ribbon.png`。依赖准备完成后，运行 `node scripts/generate-icons.mjs` 可离线导出透明 PNG 与包含 16–256 像素九种尺寸的 Windows ICO；重新编译并重启应用后托盘生效。完整提示词见 [素材说明](docs/character-assets.md)。

## 后续加入 DeepSeek 插件

直接增加 `packages/deepseek-plugin/`，定义自己的 `package.json`（例如 `@silverwolf/deepseek-plugin`），通过 `"@silverwolf/core": "workspace:*"` 引用共享契约。

`@silverwolf/core` 提供 `UsageProvider`、`UsageSnapshot`、`QuotaMeter`、`BalanceMeter`。DeepSeek 后端可从宿主凭据服务查询余额，并返回 `kind: 'balance'` 的数据；Codex 当前返回 `kind: 'quota'`。金额使用十进制字符串，避免把货币和百分比混为同一种值。

DSH 插件的挂载入口、`cordis.patch.yml`、凭据访问和网页注入留在该插件包；Windows 跟随代码留在桌面应用。共享包当前为 workspace 内源码消费，未来发布 npm 包时应增加独立构建与打包配置，不能直接发布包含未构建 TypeScript 的宿主入口。

## 验证

无需安装第三方依赖即可运行：

```powershell
node --experimental-strip-types --test packages/core/src/format.test.ts
```

环境准备完成后：

```powershell
pnpm lint
pnpm typecheck
pnpm test
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
pnpm build
```

Rust 测试覆盖握手顺序、忽略通知、API Key 登录提前退出、敏感错误脱敏、超大协议帧、动态周期、缺少重置时间和多屏坐标。Node 测试覆盖显示周期与快照过期。

真机验收清单见 [docs/windows-validation.md](docs/windows-validation.md)。窗口跟随识别 `Codex.exe`，以及微软商店 `OpenAI.Codex_*__2p2nqsd0c76g0` 目录内改名后的 `ChatGPT.exe`；窗口类需以 `Chrome_WidgetWin` 开头。普通同名 ChatGPT 程序不会被这个兼容分支匹配。混合 DPI、多窗口、最大化与遮挡必须实测。

## 参考

- [Codex App Server](https://learn.chatgpt.com/docs/app-server)
- [Tauri Windows prerequisites](https://v2.tauri.app/start/prerequisites/)
- GPT-Niang、DeepSeek-Balance-Whale-Widget、Codex-Whale-Public 用于架构调研；本项目未复制其源码。当前角色来自用户提供的参考图，经透明背景处理；项目与托盘图标基于角色头上的缎带生成。素材来源和处理提示词见 [docs/character-assets.md](docs/character-assets.md)。
