# CodexCapsule — Codex 用量胶囊小组件

一个 100×30 的桌面胶囊小部件，实时显示本机 **Codex CLI**（ChatGPT 账号登录）的用量剩余情况。

## 功能

- **胶囊窗口**：100×30 无边框全圆角小组件，浅灰半透明底色，置顶显示
- **整窗拖动**：按住窗口任意位置即可拖动；拖动结束时自动**贴边吸附**（靠近屏幕四边/四角自动吸附到边缘，避开任务栏），重启后记住位置
- **左侧**：5 小时窗口剩余用量百分比（纯数字，不含 %）；背景以**白色**填充剩余量（剩余多少填多少，用掉部分透明露出底色，用光=全透明，满格=整条白色）
- **中间**：当前套餐版本（Plus / Pro / Free …）
- **右侧**：周窗口剩余用量百分比（纯数字，不含 %）；背景以**红色**填充剩余量（逻辑同上，满格=整条红色）
- **点击左侧**：临时显示 5 小时窗口重置的准确时间（如 `20:17`，跨天自动带日期如 `9/21 02:00`），5 秒后自动恢复
- **点击右侧**：临时显示周窗口重置的准确时间（如 `9/26 22:27`），5 秒后自动恢复
- **自动刷新**：每 5 分钟从 OpenAI 后端重新拉取一次用量

## 技术栈

- **前端**：Vue 3 + TypeScript + Vite
- **桌面框架**：Tauri 2（Rust）
- **数据来源**：本机 `~/.codex/auth.json` 中的 ChatGPT 登录凭据，调用
  `https://chatgpt.com/backend-api/wham/usage` 获取用量（与 Codex CLI `/status` 同源）

## 数据说明

| 数据项 | 来源 |
|---|---|
| 套餐类型（plus/pro/free…） | 用量接口 `plan_type` 字段 |
| 5 小时剩余百分比 | `rate_limit.primary_window.used_percent`，剩余 = 100 − 已用 |
| 周剩余百分比 | `rate_limit.secondary_window.used_percent`，剩余 = 100 − 已用 |
| 重置时间 | `reset_at`（Unix 时间戳）与 `reset_after_seconds` |

- 凭据仅在本机 Rust 后端使用，**不会**发送到前端或第三方。
- 若登录过期（HTTP 401），窗口会显示错误提示，运行 `codex login` 或打开一次 Codex 应用即可刷新登录。
- 贴边吸附基于 Windows 显示器工作区（自动避开任务栏），阈值 24 逻辑像素。

## 使用

```bash
# 安装依赖
npm install

# 开发模式（热更新）
npm run tauri dev

# 构建安装包
npm run tauri build
```

构建产物：

- 可执行文件：`src-tauri/target/release/codex-capsule.exe`
- Windows 安装包：`src-tauri/target/release/bundle/nsis/CodexCapsule_0.1.0_x64-setup.exe`

## 项目结构

```
codexcApsule/
├── src/                    # Vue3 前端
│   └── App.vue             # 胶囊 UI（数值、点击交互、拖动）
├── src-tauri/
│   ├── capabilities/       # Tauri 权限配置
│   ├── icons/              # 应用图标
│   ├── src/
│   │   ├── usage.rs        # 读取 auth + 调用用量 API
│   │   ├── window_pos.rs   # 窗口位置记忆
│   │   └── lib.rs          # 应用入口
│   └── tauri.conf.json     # 窗口配置（100×30、无边框、置顶）
```

## 已知限制

- 依赖本机 `~/.codex/auth.json`，未登录 Codex 时无法获取数据。
- access token 约 1 小时过期，过期后需运行 `codex login` 或打开 Codex 应用刷新（Codex 每次运行会自动刷新 token）。
