# ksh_analyze

ksh_analyze 是一个用于分析和构建《饥荒联机版》着色器文件的工具。

## 安装

首先，确保你已经安装了 Rust 和 Cargo。然后在项目根目录下运行以下命令来构建项目：

```sh
cargo build --release
```

## 使用

### 解析 ksh 文件

将 .ksh 文件分析并转换为 .yaml 文件：

```sh
ksh_analyze input.ksh
```

指定输出目录：

```sh
ksh_analyze input.ksh output_dir
```

### 从yaml文件构建ksh文件

将 .yaml 文件构建为 .ksh 文件：

```sh
ksh_analyze input.yaml
```

指定输出目录：

```sh
ksh_analyze input.yaml output_dir
```

### 启用调试日志

启用调试日志以获得更详细的输出：

```sh
ksh_analyze input.ksh --debug
```

## 构造ksh的yaml解构

以下是 .yaml 文件的基本结构：

```yaml
version: 1
shaders:
  - vs:
      file: <顶点着色器文件路径>
      uniforms:
        - <顶点着色器使用的uniform变量名>
    ps:
      file: <片段着色器文件名>
      uniforms:
        - <片段着色器使用的uniform变量名>
    uniforms:
      - name: <uniform变量名>
        type: <uniform变量类型>
        array_length: <数组长度，可选>
        default_data: <默认数据，可选>
```
**字段说明**
* shaders: 着色器配置列表。
* vs: 顶点着色器配置。
  * file: 顶点着色器文件路径。
  * uniforms: 顶点着色器使用的 uniform 变量名列表。
* ps: 片段着色器配置。
  * file: 片段着色器文件路径。
  * uniforms: 片段着色器使用的 uniform 变量名列表。
* uniforms: uniform 变量配置列表。
  * name: uniform 变量名。
  * type: uniform 变量类型，可能的值包括 float, vec2, vec3, vec4, mat4, sampler2D。
  * array_length: 数组长度（可选），如果是数组类型则需要指定。
  * default_data: 默认数据（可选），如果有默认值则需要指定。

**一个完整的yaml例子：**

anim.ksh -> anim.yaml, anim.ps, anim.vs

```yaml
shaders:
- vs:
    file: anim.vs
    uniforms:
    - MatrixP
    - MatrixV
    - MatrixW
    - TIMEPARAMS
    - FLOAT_PARAMS
  ps:
    file: anim.ps
    uniforms:
    - SAMPLER
    - LIGHTMAP_WORLD_EXTENTS
    - COLOUR_XFORM
    - PARAMS
    - FLOAT_PARAMS
    - OCEAN_BLEND_PARAMS
    - OCEAN_WORLD_EXTENTS
  uniforms:
  - name: MatrixP
    type: mat4
  - name: MatrixV
    type: mat4
  - name: MatrixW
    type: mat4
  - name: TIMEPARAMS
    type: vec4
  - name: FLOAT_PARAMS
    type: vec3
  - name: SAMPLER
    type: sampler2D
    array_length: 5
  - name: LIGHTMAP_WORLD_EXTENTS
    type: vec4
  - name: COLOUR_XFORM
    type: mat4
  - name: PARAMS
    type: vec3
  - name: OCEAN_BLEND_PARAMS
    type: vec4
  - name: OCEAN_WORLD_EXTENTS
    type: vec4

```
## 为什么构造ksh需要yaml来声明?

本来计划直接读取ps vs文件来解析的, 尝试用crate glsl解析ast, 发现支持不够完美. 为了避免可能存在的解析失败的情况, 采用yaml来声明ksh的结构. (例如, anim.ksh 里在块级里使用预处理指令就会解析失败)

## 贡献

欢迎贡献代码！请 fork 本仓库并提交 pull request。

## 许可证

此项目使用 BSD 3-Clause 许可证。详情请参阅 [LICENSE](./LICENSE) 文件。
