# GitHub Actions 构建说明

## 工作流说明

此项目使用 GitHub Actions 自动构建和发布安装包。

### 工作流文件

- `.github/workflows/build-release.yml` - 主要构建和发布工作流
- `.github/workflows/pr-check.yml` - PR 检查工作流

### 构建配置

项目支持以下目标平台的交叉编译：

- `x86_64-unknown-linux-gnu` - 64位 x86 Linux
- `aarch64-unknown-linux-gnu` - 64位 ARM Linux
- `armv7-unknown-linux-gnueabihf` - 32位 ARM Linux

### 构建流程

1. 检出代码（包含子模块）
2. 构建前端（Vue.js）
3. 安装交叉编译工具链
4. 编译 Rust 代码
5. 创建发布资产
6. 上传构建产物

### 发布说明

当创建带有 `v` 前缀的标签时（如 `v0.1.0`），会触发发布流程，自动生成预构建的二进制文件并创建 GitHub Release。

### 静态构建

工作流同时构建常规版本和静态链接版本：
- 常规版本：较小，但可能需要系统库
- 静态版本：包含所有依赖，可在任何兼容系统上运行

## 本地构建

如果要在本地构建项目，请确保安装了以下依赖：

- Rust 工具链
- Node.js (用于前端构建)
- 交叉编译工具链
- 系统依赖（pkg-config, libssl-dev, libasound2-dev 等）

然后运行：

```bash
# 构建前端
cd web && npm install && npm run build && cd ..

# 构建后端
cargo build --release
```