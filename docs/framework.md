# 通用角色挂件

从角色旁的设置按钮进入快捷设置，选择「外观、内容与外部推送…」打开完整设置；托盘也可以直接打开。完整设置的修改点击「保存设置」后生效。

## 内容与展示

日常内容源只能选择一个：自己的文字与图片、一言、自定义 GET API 或 Codex 额度。文本列表可以排序、随机取句和添加图片。点击角色换一句；Codex 模式点击仅展开，额度在可见时每 60 秒刷新。非额度内容可开启定时换句（10–86400 秒）。

外部消息进入 FIFO 队列并临时接管气泡。图片下载失败会标记失败并继续下一条。跟随 Codex 且窗口隐藏时不开始计时；悬停、键盘操作、加载和设置打开期间暂停，恢复后重新完整计时。队列结束后恢复原来的展开状态。关闭当前消息进入下一条；完整设置中的「清空推送队列」结束整批消息。

新用户默认在桌面显示本地欢迎文本。旧配置自动备份为 `settings.v1.backup.json`，并迁移到版本 2；原有跟随、Codex 路径、位置和显示时长保留。应用标识与原配置目录不变。

## 自定义 API

请求由 Rust 发出，支持 GET、查询参数和请求头。选择纯文本响应，或用 JSON Pointer 映射文字、作者、出处和图片 URL。例如响应 `{"data":{"text":"你好","author":"某人"}}` 对应 `/data/text` 和 `/data/author`；数组可用 `/items/0/text`。留空的字段不读取，不运行转换脚本。

接口响应上限 256 KB，请求超时 10 秒，最多跟随 3 次重定向。图片单独下载验证，接口返回的文字不会作为 HTML 执行。请求头存于本机设置文件，不发送给模板。

## 本机 HTTP 推送

在「外部推送」中开启并保存，默认地址为 `http://127.0.0.1:38427`。所有接口需要 `Authorization: Bearer <令牌>`；设置中可显示或复制令牌。令牌保存在配置目录的 `http-token`，不要加入主题包。服务不允许网页跨域调用，Host 仅接受本机地址。

PowerShell 示例：

```powershell
$headers = @{ Authorization = 'Bearer 替换为设置中的令牌' }
$body = @{ text = '休息一下吧'; durationSeconds = 8 } | ConvertTo-Json
$message = Invoke-RestMethod -Method Post -Uri http://127.0.0.1:38427/v1/messages -Headers $headers -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body))
Invoke-RestMethod -Uri "http://127.0.0.1:38427/v1/messages/$($message.id)" -Headers $headers
```

| 方法与路径 | 请求 / 返回 |
| --- | --- |
| `POST /v1/messages` | JSON：可选 `title`、`text`、`image`、`durationSeconds`；文字或图片至少一个。返回 `202 {id, accepted:true}` |
| `POST /v1/assets` | multipart 文件上传，返回 `{assetId}`；支持 PNG/JPEG/WebP/GIF |
| `GET /v1/messages/{id}` | 返回消息内容、时长、`status`、`error` |
| `GET /v1/status` | 返回 `running`、`port`、`error`、`visible`、`queueLength` |

图片字段二选一：`{"image":{"url":"https://example.com/picture.png"}}`，或 `{"image":{"assetId":"上传返回的标识"}}`。不接受本机文件路径、HTML 或脚本；文字和图片可以同时提供。

状态为 `preparing`（下载图片）、`waiting`、`showing`、`completed`、`failed`。`202` 表示已接收，不代表已经展示。待展示队列最多 100 条，超限返回 `429`；展示时长默认 8 秒，允许整数 1–3600。文字最多 32000 UTF-8 字节，标题最多 256 字节。图片最大 8 MB、1600 万像素；临时素材空间上限 256 MB，重启清理。队列不持久化，完成状态最近保留 200 条，过期或重启后返回 `404`。

## 主题包

参考 `examples/paper-theme`。可直接导入这个目录，或将目录打成 ZIP。应用复制素材到自己的数据目录，原文件移动不会影响已导入主题。人物和音效也可分别导入，覆盖主题中的默认值。

`theme.json`：

```json
{
  "schemaVersion": 1,
  "name": "我的角色",
  "character": "character.png",
  "dialog": "dialog.html",
  "sound": "click.wav"
}
```

三个资源字段均可用 `null`；人物为空使用内置人物，对话框为空使用内置云朵，音效为空不播放。对话框入口必须为 `.html`。可省略 `layout` 使用内置尺寸，完整字段见示例主题：画布 `width/height`、人物和气泡的 `x/y/width/height`，以及工具栏的 `x/leftX/y`。画布范围 376×430 至 800×1000，布局必须位于画布内。前后端共享默认布局文件，原生窗口尺寸和点击区域跟随主题布局。

ZIP/目录最多 128 个文件、合计 32 MB。禁止绝对路径、路径越界、符号链接和 Windows 特殊文件名。主题资源只使用包内相对路径；禁止外部脚本、网络连接、表单提交、嵌套框架和弹窗。人物支持 PNG/JPEG/WebP/GIF，音效支持 MP3/WAV/OGG。

## JavaScript 模板接口 v1

宿主自动注入 `window.Silverwolf`。无需安装 SDK；建议使用 `defer` 加载包内脚本。

```javascript
const unsubscribe = window.Silverwolf.onContent((content, state, theme) => {
  document.querySelector('#text').textContent = content?.text || ''
  // content: id/title/text/image/author/source/observedAt/usage
  // state: expanded/loading/error
  // theme: id/name/layout（仅展示参数）
})
// 用户操作时调用：
window.Silverwolf.refresh()
window.Silverwolf.close()
```

`image` 为宿主管理的图片 URL，`usage` 为原额度快照或 null。使用 `textContent` 处理来自接口和推送的文字。模板只能收到展示数据，不收到配置、CLI 路径、请求头或访问令牌。键盘交互与渲染就绪由注入的桥接脚本报告。

模板运行于独立资源源的 `sandbox="allow-scripts"` iframe，没有同源权限或 Tauri 命令权限。父窗口同时校验 iframe、会话和消息结构。握手或渲染超过 5 秒、脚本报告错误时回退到内置气泡；托盘的「恢复默认外观」可恢复角色与对话框。

模板资源由仅监听回环地址的随机端口提供，每次启动使用新的随机资源前缀；它与可选的推送服务独立，不提供设置、文件或命令接口。模板未注册为 Tauri 本地协议，能力白名单不授权此来源。网络请求仍由模板 CSP 阻止。

隔离设计参考 [MDN iframe sandbox](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/iframe) 和 [Tauri capabilities](https://v2.tauri.app/security/capabilities/)。[一言接口](https://developer.hitokoto.cn/sentence/)字段为 `hitokoto`、`from_who`、`from`，分类参数使用重复的 `c`。

## 开发验证

沿用根目录 `pnpm lint`、`pnpm typecheck`、`pnpm test`、`cargo test --workspace --locked`、`cargo clippy --workspace --all-targets -- -D warnings` 与 `pnpm build:web`。Rust 新增 Axum、Reqwest、ZIP、图片验证等依赖，锁文件已同步。无需 Node 后端或浏览器 HTTP 插件。

调试构建可使用绝对路径环境变量 `SILVERWOLF_DEV_DATA` 指向隔离配置和素材目录；发布构建忽略该变量。`cargo build -p silverwolf-desktop --features tauri/custom-protocol --offline --locked` 可生成内嵌前端的调试程序。先执行 `pnpm build:web` 更新内嵌资源。

隔离实例开启 HTTP 后，可运行 `node scripts/smoke-http.mjs http://127.0.0.1:端口 隔离目录/http-token drain` 验证鉴权、参数校验、上传、图片失败、FIFO 与自动完成。鼠标需移出气泡和工具栏，且关闭设置。将末尾改为 `paused` 并打开设置，可验证暂停和 100 条满载；测试后在设置清空队列。脚本会发送测试内容，仅用于隔离实例，不打印令牌。

首版仅支持 Windows 单角色、单主内容源；不包括插件市场、公网接收、多角色或跟随任意应用。
