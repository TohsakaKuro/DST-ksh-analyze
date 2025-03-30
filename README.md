# ksh_analyze

ksh_analyze 是一个用于分析和构建《饥荒联机版》着色器文件的工具。它可以直接从 .ksh 文件中提取着色器内容，也可以从 .vs 和 .ps 文件构建 .ksh 文件。

## 功能说明

### 解析 ksh 文件
- 从 .ksh 文件中提取顶点着色器（.vs）和像素着色器（.ps）的内容

### 构建 ksh 文件
- 支持从包含着色器文件的目录构建
- 支持从两个独立的着色器文件构建
## 源码构建

首先，确保你已经安装了 Rust 和 Cargo。然后在项目根目录下运行以下命令来构建项目：

```sh
cargo build --release
```

## 直接下载使用

直接下载release版本使用, 你可以把他适当的添加到你的环境变量

## 使用

### 命令

```
用法: ksh_analyze.exe [OPTIONS] <输入路径> [第二个文件] [输出目录或文件]

Arguments:
  <输入路径>     输入路径，可以是：
             - .ksh 文件（用于分析）
             - 包含 vs 和 ps 着色器文件的目录
             - 两个着色器文件（vs 和 ps，顺序任意）
  [第二个文件]    第二个着色器文件路径。仅在输入为着色器文件时需要。
  [输出目录或文件]  输出路径。如果未指定，将使用当前目录。

Options:
  -d, --debug    启用调试日志以获取更详细的输出。
  -f, --force    允许覆盖文件
  -h, --help     Print help
  -V, --version  Print version
```

是解析还是生成ksh文件模式, 是由命令的第一个参数决定的. 如果第一个参数是ksh的, 那么将解析. 否则生成

### 解析 ksh 文件为着色器文件
#### 将 .ksh 文件解析并提取着色器文件(.vs, .ps)：

指定输入与输出目录：

```sh
ksh-analyzer input.ksh output
```


### 从着色器文件构建 ksh 文件

#### 从包含着色器文件的目录构建：

程序会扫描输入文件夹, 自动查找里面的.vs, .ps文件, 并构建ksh文件.

构建的ksh文件中的着色器, 将以输出的文件名命名着色器.

输出允许忽略.ksh后缀, 程序会自动添加.ksh后缀.

```sh
ksh-analyzer shader_dir output.ksh
```

#### 从两个着色器文件构建（顺序任意）：

```sh
ksh-analyzer input.vs input.ps output.ksh
```

### 命令行选项

- `-d, --debug`: 启用调试日志以获取更详细的输出
- `-f, --force`: 允许覆盖已存在的输出文件

### 计划

✅ 解析与生成ksh文件
✅ 移除yaml格式的配置
❌ 除了命令行, 额外支持ui界面

## 贡献

欢迎贡献代码！请 fork 本仓库并提交 pull request。

## 许可证

此项目使用 BSD 3-Clause 许可证。详情请参阅 [LICENSE](./LICENSE) 文件。
