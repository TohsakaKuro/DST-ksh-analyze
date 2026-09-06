# KSH 解析与构建阶段报告

日期：2026-09-07。

本文保留解析与构建阶段当时的修复和验证记录。后续前端已拆分文档模型与文件操作模块，当前行为及测试入口以 [README](../README.md) 和 [工作台架构](workbench-architecture.md) 为准；下方测试数量不代表当前工作区的最新总数。

## 本阶段范围

补全解析、构建和编辑器文件操作逻辑，为后续编辑器内 shader 预览准备基础。本阶段不启动游戏，不加入预览功能，不提交或回退工作区的其他修改。

## 已完成的修复

- 严格读取 KSH 到文件末尾，保留 effect、uniform 元数据、默认值原始位、VS/PS 源码及阶段索引；未知 scope/type 可无损往返。
- 补齐 `0..45` 类型码定义；`1/22` 标记为保留位。sampler `42..45` 不读写数值默认值块。
- 完善 GLSL 类型映射和保守默认值求值，按 VS first-wins 合并跨阶段默认值；删除源码 initializer 后归零，不沿用旧初始化值。
- 区分标量与单元素数组，修复默认值块布局误判；使用词法作用域识别 uniform 引用，避免局部变量或参数遮蔽导致误分配 sampler 槽位。
- 修复 release 专属作用域错误：弹出作用域不再放在 `debug_assert!` 内，避免发布版跳过实际操作。
- 检查 Windows 非法和设备文件名。解包先暂存 VS/PS，再成对提交；普通 I/O 失败时回滚已有文件。
- 完善内容与名称的修改状态、保存快照和串行写入；旧读取结果不会覆盖读取期间的新编辑，确认队列会重新扫描新增修改。
- 仅在成功读取且快照仍有效、即将应用新内容时作废旧保存，修复打开失败导致并行 Save As 丢失新路径和保存点的问题。

## 验证结果

| 检查 | 结果 |
| --- | --- |
| Rust release 全目标、全特性测试 | 51 个库测试 + 1 个 CLI 测试通过 |
| 官方 KSH 二进制读取/编码往返 | 本机 65 个样本逐字节一致 |
| 官方 KSH 从源码重新构建往返 | 本机 65 个样本逐字节一致 |
| 前端文件操作回归 | 31 个测试通过 |
| Rust 格式与 Clippy 严格检查 | 通过 |
| 前端生产构建 | 通过，保留既有 Monaco 包体积警告 |

本阶段原有前端测试位于 `tests/editor-file-operations.test.mjs`，当时从真实 `App.vue` 提取脚本并执行原函数，仅替换外部依赖，覆盖打开与保存交错、读取期间编辑和确认队列变化。工作台重构后，该文件已迁移为文档模型测试，文件流程另由 `tests/document-actions.test.mjs` 覆盖；复跑命令见 README。

官方样本检查依赖 `src-tauri/src/core.rs` 中的本机样本路径；目录不存在时会跳过，不能把跳过视为样本验证成功。

## 官方编译器参考

实际工具位置与用户最初提供的目录拼写略有不同：

```text
C:\Saved Games\Steam\steamapps\common\Don't Starve Mod Tools\mod_tools\tools\bin\ShaderCompiler.exe
```

已确认隐藏入口为 `ShaderCompiler.exe -little <effect> <vertex.cg> <pixel.cg> <output.ksh>`。这是旧 Cg 前端，不应直接接为当前 GLSL 编辑器的默认编译后端。

通用研究与可复用工具已保存在共享仓库，不在本项目重复维护：

- `C:\Users\Tohsa\projects\DST-Arknights-AICoding\research\dst-shaders\official-shader-compiler.md`
- `C:\Users\Tohsa\projects\DST-Arknights-AICoding\research\dst-shaders\tools\Invoke-OfficialShaderCompilerProbe.ps1`
- `C:\Users\Tohsa\projects\DST-Arknights-AICoding\research\dst-shaders\tools\ksh_inspect.py`

探针已验证 PowerShell 5.1 和 7：成功提交、默认拒绝覆盖、显式替换、失败保留旧输出、未知工具指纹门禁、结构损坏拒绝、UTF-8 输出与相对路径解析。

真实 `float1`、`int1` 实验证明，官方工具可能返回退出码 0，却报告 `Unexpected type passed to ConvertCgType: 1091` / `1094` 并漏写 uniform。生成文件仍可通过严格结构检查。探针现会拦截已知诊断，两版本均验证已有输出哈希不变；一般 warning 不会被一概拒绝。结构检查不能证明 uniform 语义完整。

本轮探针测试和样本另保留在 `C:\Users\Tohsa\Documents\Codex\2026-09-04\new-chat\probe-smoke`，其中 `Test-CompilerProbe.ps1` 覆盖常规行为，`Test-UnsupportedUniformProbe.ps1` 覆盖已知漏 uniform 行为。

## 已知边界与下一步

- 当前构建器生成 KSH 容器，不是完整的 GLSL 驱动编译器。复杂 initializer、异型矩阵转换等无法可靠确定语义的表达式会拒绝；未调用函数中的 uniform 仍保守保留。
- 能编码某个类型不等于当前游戏能运行该类型。后续预览优先针对 GLES2 实用类型：`float`、`vec2/3/4`、`mat2/3/4`、`sampler2D`、`samplerCube`。
- 成对文件提交支持普通 I/O 失败回滚，不是断电或进程崩溃时的双路径原子事务。
- 尚未做原生窗口、真实文件对话框和 Monaco 手工交互验证，也没有游戏运行验证。
- 后续预览实验可研究 GLSL 编译/链接、纹理输入和 uniform 参数；这些能力尚未进入当前源码工作台。预览结果与游戏特有管线的差异需要明确标注，不能把预览成功当作游戏兼容性证明。
