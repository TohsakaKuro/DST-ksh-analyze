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
- 内置代码编辑器，支持 GLSL 语法高亮
- 支持 VS/PS 标签页切换编辑
- 支持从 KSH 文件导入和导出
- 导入后再导出时保留兼容的 effect、uniform 默认值、未知类型/scope 和阶段索引
- 支持独立保存 VS/PS 文件
- 支持代码注释、撤销/重做等编辑功能

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
# 前端文件操作回归测试，直接执行 App.vue 中的真实逻辑
node --test tests/editor-file-operations.test.mjs

# Rust 测试；release 模式也需检查，避免依赖 debug_assert 的副作用
cargo test --manifest-path src-tauri/Cargo.toml --all-targets --all-features
cargo test --release --manifest-path src-tauri/Cargo.toml --all-targets --all-features

# 格式、静态检查和前端生产构建
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
npm run build
```

Rust 测试包含合成格式样例和本机官方 KSH 样本的逐字节往返检查。官方样本目录不存在时，该部分会跳过；普通测试通过不表示已检查官方样本。前端测试模拟文件对话框、文件系统和编辑器依赖，不代替原生界面手工测试。以上检查均不启动游戏，也不能证明 shader 在游戏中能够编译、链接和正确渲染。

本阶段修复、验证结果与后续预览器边界见 [阶段报告](.Codex/parser-builder-validation-2026-09-07.md)。

## 直接下载使用

直接下载 release 版本使用。双击运行程序即可使用图形界面。

## 使用方式

### 图形界面

直接双击 `dst-ksh-analyze`，将打开图形界面。界面主要功能：

1. 文件操作
   - 从 KSH 导入：打开 KSH 文件并提取着色器代码
   - 导出到 KSH：将当前编辑的着色器代码保存为 KSH 文件
   - 打开/保存：独立打开或保存 VS/PS 文件

2. 编辑功能
   - 支持 GLSL 语法高亮
   - 代码注释/取消注释（Ctrl + /）
   - 撤销/重做（Ctrl + Z / Ctrl + Y）
   - 查找/替换（Ctrl + F）

3. 界面特性
   - VS/PS 标签页切换
   - 支持修改着色器名称
   - 文件修改状态提示
   - 保存提醒对话框

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

✅ 解析与生成ksh文件
✅ 移除yaml格式的配置
✅ 除了命令行, 额外支持ui界面
❌ 编辑器内实时渲染着色器预览
❌ 支持着色器代码格式化
✅ 支持着色器语法检查

## 相关组件

本项目使用了以下开源组件：

### 前端组件
- **Vue.js** (v3.5.13) - 渐进式JavaScript框架
  - 许可证: MIT
  - 项目地址: https://github.com/vuejs/vue

- **Monaco Editor** (v0.52.2) - 基于VS Code的代码编辑器
  - 许可证: MIT
  - 项目地址: https://github.com/microsoft/monaco-editor

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
