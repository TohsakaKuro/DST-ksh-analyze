# dst-ksh-analyze

dst-ksh-analyze 是一个用于分析和构建《饥荒联机版》着色器文件的工具。它可以直接从 .ksh 文件中提取着色器内容，也可以从 .vs 和 .ps 文件构建 .ksh 文件。

## 仓库地址

[仓库地址: https://github.com/TohsakaKuro/DST-ksh-analyze](https://github.com/TohsakaKuro/DST-ksh-analyze)

欢迎贡献代码！请 fork 本仓库并提交 pull request。


## 功能说明

### 解析 ksh 文件
- 从 .ksh 文件中提取顶点着色器（.vs）和像素着色器（.ps）的内容

### 构建 ksh 文件
- 支持从包含着色器文件的目录构建
- 支持从两个独立的着色器文件构建

### 图形界面功能
- 内置代码编辑器，支持 GLSL 语法高亮
- 实时编辑和预览着色器代码
- 支持从 KSH 文件导入和导出
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

## 直接下载使用

直接下载 release 版本使用。双击运行程序即可使用图形界面。

## 使用方式

### 图形界面

直接双击运行程序，将打开图形界面。界面主要功能：

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
   - 双栏布局，左侧 PS 右侧 VS
   - 支持修改着色器名称
   - 文件修改状态提示
   - 保存提醒对话框

### 计划

✅ 解析与生成ksh文件
✅ 移除yaml格式的配置
✅ 除了命令行, 额外支持ui界面
❌ 支持着色器代码格式化
❌ 支持着色器语法检查

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
