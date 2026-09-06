# dst-ksh-analyze

dst-ksh-analyze 是一个用于分析和构建《饥荒联机版》着色器文件的工具。它可以直接从 .ksh 文件中提取着色器内容，也可以从 .vs 和 .ps 文件构建 .ksh 文件。

## 仓库地址

[仓库地址: https://github.com/TohsakaKuro/DST-ksh-analyze](https://github.com/TohsakaKuro/DST-ksh-analyze)

欢迎贡献代码！请 fork 本仓库并提交 pull request。


## 功能说明

### 解析 ksh 文件
- 从 .ksh 文件中提取顶点着色器（.vs）和像素着色器（.ps）的内容
- 严格解析 effect、完整 uniform 表、默认值原始位和 VS/PS 索引，并检查文件是否恰好解析到末尾
- 识别官方格式的 `0..45` 类型码；未知 scope/type 也可无损读取和重新编码
- 解包时检查 Windows 可移植文件名，并把 VS/PS 作为一组暂存、提交；普通 I/O 失败会回滚旧文件

### 构建 ksh 文件
- 支持从包含着色器文件的目录构建
- 支持从两个独立的着色器文件构建
- 支持 GLSL 标量、向量、方阵/非方阵、整数向量和四类 sampler 的 KSH 类型映射
- 支持把数值字面量及数值构造器形式的 uniform 初始化值写成 KSH float32 默认值
- 按 GLSL 词法作用域识别实际引用的 uniform；局部变量和函数参数遮蔽不会误占 sampler 槽位
- 区分标量与单元素数组，避免 `float X` 和 `float X[1]` 共用错误的默认值布局

这里的“构建”是生成当前游戏发布资产所用的 GLSL KSH 容器，不会调用 Mod Tools 中面向 Cg 的旧 `ShaderCompiler.exe`。复杂常量表达式若无法可靠求值会返回错误，不会静默改写成零。

当前构建器不是显卡驱动的完整 GLSL 语义编译器。它会保守保留只在未调用函数中出现的 uniform；异型矩阵转换、声明类型不匹配的构造器和无法确定语义的初始化表达式会明确拒绝，最终 shader 编译与链接仍需由游戏或后续预览器验证。

KSH 的“可编码类型”也不等于当前 DST 的“可运行类型”。当前 Windows 客户端动态上传只确认类型码 `0..8`、`10`、`15`、`20`，sampler `42..45` 走另一条绑定路径；但当前 GLES2 源码实际应优先使用 `float`、`vec2/3/4`、`mat2/3/4`、`sampler2D` 和 `samplerCube`。整数类型、非方阵及 `sampler1D/3D` 可用于容器研究和无损处理，不应在没有运行验证时当作可用的模组 shader 类型。

### 图形界面功能

- 启动时为空，使用 Fluent UI 菜单与按钮、独立文件标签和 Monaco 编辑器
- 可同时打开多份 `.vs`、`.ps`、`.glsl`、`.txt` 源码；打开 KSH 会追加两份独立源码标签
- `Ctrl+S` 保存当前文件，另有“全部保存”；每个标签可单独关闭，修改标记和未保存确认按文件管理
- 导出时从已打开文件中各选一个 VS 和 PS，再通过系统保存窗口确定输出文件名；无需填写内部名称
- 同源 KSH 的原始 VS/PS 组合保留兼容的 uniform 默认值、未知类型/scope 和阶段索引；跨来源组合在确认后重建元数据
- 支持任意两个已打开文件并排编辑，不固定绑定 VS/PS；切换标签保留各自撤销历史和光标位置
- 支持代码注释、撤销/重做、查找和替换

当前界面集中于源码编辑与文件操作，不包含 uniform 参数面板、诊断面板或编辑时的实时语法检查。打开、保存和构建失败仍会显示错误信息；编辑器内实时渲染预览尚未实现。

## 源码构建

首先，确保你已经安装了以下依赖：
- Node.js 和 npm
- Rust 和 Cargo

然后在项目根目录下运行以下命令来构建项目：

```sh
# 安装前端依赖
npm install

# 构建发布版本
npm run tauri build
```

## 开发验证

在项目根目录运行：

```sh
# 文档状态与文件操作流程回归测试
node --test tests/editor-file-operations.test.mjs tests/document-actions.test.mjs

# Rust 测试；release 模式也需检查，避免依赖 debug_assert 的副作用
cargo test --manifest-path src-tauri/Cargo.toml --all-targets --all-features
cargo test --release --manifest-path src-tauri/Cargo.toml --all-targets --all-features

# 格式、静态检查和前端生产构建
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
npm run build
```

Rust 测试包含合成格式样例、独立 CLI 进程测试和本机官方 KSH 样本的逐字节往返检查。官方样本目录不存在时，该部分会跳过；普通测试通过不表示已检查官方样本。前端测试直接验证文档状态模块，并通过模拟文件接口和对话框验证文件操作流程，不代替原生界面手工测试。以上检查均不启动游戏，也不能证明 shader 在游戏中能够编译、链接和正确渲染。

可选的浏览器交互测试使用真实 Vue、Fluent 和 Monaco，仅模拟原生对话框及文件 IO，不读写真实着色器文件。运行环境需要能解析 `playwright` 模块，并已安装 Microsoft Edge：

```sh
# 第一个终端：构建并启动页面
npm run build
npm run preview -- --host 127.0.0.1 --port 1420
```

```sh
# 页面启动后，在另一个终端运行
node tests/workbench-ui-smoke.cjs
```

可通过环境变量调整：`PLAYWRIGHT_MODULE` 指向现有 Playwright 模块的绝对路径，`SHADER_UI_URL` 指定页面地址（默认 `http://127.0.0.1:1420`），`SHADER_UI_BROWSER` 指定已安装的浏览器通道（默认 `msedge`）。截图写入 `SHADER_UI_OUTPUT` 指定目录；未指定时在系统临时目录下创建独立目录。端口已占用时为页面选择其他端口，并同步设置 `SHADER_UI_URL`。

解析与构建的历史验证记录见 [阶段报告](.Codex/parser-builder-validation-2026-09-07.md)，当前前端模块边界与预览接入约定见 [工作台架构](.Codex/workbench-architecture.md)。

## 直接下载使用

直接下载 release 版本使用。双击运行程序即可使用图形界面。

## 使用方式

### 图形界面

双击 `dst-ksh-analyze` 启动空白工作台。主界面以独立源码文件为单位，不维护一个固定的 KSH 文档或 VS/PS 配对。

1. 使用 `Ctrl+N` 新建未命名文件，或通过“文件 → 打开文件”/ `Ctrl+O` 同时打开多份源码和 KSH。打开操作追加标签，不替换已有编辑；重复打开同一路径会切换到原标签。
2. 标签显示源码文件名、修改标记和关闭按钮。`Ctrl+Tab` / `Ctrl+Shift+Tab` 切换标签，`Ctrl+W` 或鼠标中键关闭标签；“文件 → 关闭全部”关闭所有文件。并排时点击编辑区确定焦点，再选标签可切换该侧文件。
3. `Ctrl+S` 只保存当前文件，`Ctrl+Shift+S` 另存为；“全部保存”逐一保存未保存的文件。未命名文件通过系统窗口选择路径，未指定扩展名时使用 `.glsl`。源码支持 `.vs`、`.ps`、`.glsl` 和 `.txt`，不会因语法尚未完成而拒绝保存。
4. 使用“导出 KSH”或 `Ctrl+Shift+E`，在两个单选列表中各选一份 VS、PS，然后选择输出路径。`.vs` / `.ps` 默认按扩展名分类，通用 `.glsl`、`.txt` 和未命名文件可按需分配阶段；同一个文件不能同时承担两阶段。已有多个候选时需明确选择。

导出读取所选标签的当前内容，不要求先保存源码，但不会清除源码修改标记。关闭有未保存内容的标签时可保存、放弃或取消；打开 KSH 所产生的非空源码标签尚无独立保存路径，也会提醒保存。空白未命名文件可直接关闭。“全部保存”中途取消或失败时，已经成功保存的文件保持已保存，其余标签保留。

桌面导出的名称全部由输出文件名决定。例如 `glow.ksh` 自动写入 effect `glow`、内嵌名称 `glow.vs` 和 `glow.ps`，包括从 KSH 导入后再次导出的情况。源码标签及磁盘文件名不变。旧构建接口未启用 `name_from_output` 时仍保留原来的命名约定。

导入来源仅作为内部元数据关联。同一次导入的原始 VS/PS 组合会保留兼容元数据；跨 KSH、导入源码与普通文件混搭、关闭某阶段后从新一次导入补回等情况，会在导出前提醒按源码重建参数表。不会猜测合并不同 KSH 的默认值。

浏览器开发页面可用于检查布局和编辑器交互；本地文件读写及原生文件选择需要通过 Tauri 桌面应用运行。

### 命令行（CLI）

CLI 已独立为单独产物 `dst-ksh-analyze-cli`，不再与 UI 共用同一个入口。

常用示例：

```sh
# 分析 .ksh 到目录
dst-ksh-analyze-cli input.ksh output_dir

# 从目录构建 .ksh
dst-ksh-analyze-cli shader_dir output.ksh

# 从两个着色器文件构建 .ksh
dst-ksh-analyze-cli input.vs input.ps output.ksh
```

### 计划

- 已实现：KSH 解析与构建、独立 CLI、多文件源码工作台与组合导出。
- 后续实验：编辑器内实时渲染预览、着色器代码格式化。
- 当前不提供编辑时实时语法检查；构建器的解析与格式校验范围见上文。

## 相关组件

本项目使用了以下开源组件：

### 前端组件
- **Vue.js** (v3.5.13) - 渐进式JavaScript框架
  - 许可证: MIT
  - 项目地址: https://github.com/vuejs/vue

- **Monaco Editor** (v0.52.2) - 基于VS Code的代码编辑器
  - 许可证: MIT
  - 项目地址: https://github.com/microsoft/monaco-editor

- **Fluent UI Web Components** (v3.1.3) - Microsoft 的界面组件，用于菜单、按钮、对话框和消息提示
  - 许可证: MIT
  - 项目地址: https://github.com/microsoft/fluentui/tree/master/packages/web-components

- **Lucide Vue** (`@lucide/vue`, v1.41.0) - 工作台工具栏和操作按钮的图标
  - 许可证: ISC
  - 项目地址: https://github.com/lucide-icons/lucide/tree/main/packages/vue

- **Vite** (v6.0.3) - 下一代前端构建工具
  - 许可证: MIT
  - 项目地址: https://github.com/vitejs/vite

### Tauri相关
- **Tauri** (v2) - 用于构建跨平台桌面应用的框架
  - 许可证: MIT
  - 项目地址: https://github.com/tauri-apps/tauri

- **Tauri插件**
  - **tauri-plugin-dialog** (v2.2.1) - 对话框插件
  - **tauri-plugin-fs** (v2.2.1) - 文件系统插件
  - **tauri-plugin-opener** (v2.2.6) - 文件打开插件
  - 许可证: MIT
  - 项目地址: https://github.com/tauri-apps/plugins-workspace

### Rust组件
- **clap** (v4.5.32) - 命令行参数解析库
  - 许可证: MIT
  - 项目地址: https://github.com/clap-rs/clap

- **glsl-lang** (v0.7.2) - GLSL语言解析库
  - 许可证: BSD 3-Clause
  - 项目地址: https://github.com/alixinne/glsl-lang

- **serde** (v1) - 序列化/反序列化框架
  - 许可证: MIT
  - 项目地址: https://github.com/serde-rs/serde

## 许可证

此项目使用 BSD 3-Clause 许可证。详情请参阅 [LICENSE](./LICENSE) 文件。
