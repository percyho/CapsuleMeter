# 发布填写内容约定

当用户要求“生成发布填写内容”“要发布填写的内容”或类似请求时，先查看当前 `package.json` 版本、未发布改动和 `CHANGELOG.md`，再按以下规则给出建议。此类请求默认只生成内容，不自动运行打包、创建标签、创建 Release 或推送代码。

## 版本选择

- 修复、文案或样式调整，且没有新增对用户可见能力：使用 patch。
- 新增向后兼容的用户功能：使用 minor。
- 存在破坏性变更：使用 major。
- 版本号以 `package.json` 当前版本为基准，按所选类型计算下一个版本；打包脚本会同步更新 `src-tauri/tauri.conf.json`。
- 不确定变更范围或是否属于破坏性变更时，说明判断依据，不要虚构版本内容。

## 固定回复模板

每次都按以下字段顺序和标题回复。根据本次实际改动填值；不适用的产物明确标注，不要省略字段。

````markdown
### 版本与打包
- 版本类型：`patch` / `minor` / `major`（简述选择理由）
- 建议版本：`vX.Y.Z`
- 打包命令：`pnpm build:patch` / `pnpm build:minor` / `pnpm build:major`

### Release 页面填写
- Tag：`vX.Y.Z`
- 标题：`Codex Capsule vX.Y.Z`
- 描述：

```text
用中文撰写简明发行说明，概述本次主要新增、改进或修复。内容必须来自当前代码改动、未发布日志或用户提供的信息，不要添加未实现的功能。
```

### 构建产物
- Windows 安装包：`src-tauri/target/release/bundle/nsis/CodexCapsule_X.Y.Z_x64-setup.exe`
````

描述始终使用中文。若项目实际打包目标或产物路径已变化，先核对配置与构建脚本，再更新模板中的产物信息。若用户明确要求推送版本和标签，GitHub 与 Gitee 两个远程仓库都要推送。
