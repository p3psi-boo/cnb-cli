# CNB CLI Specification

CNB API 命令行接口工具。

## 核心原则

**`--help` / `-h` 是最重要的功能**。每个命令和子命令都必须提供清晰、完整的帮助信息。

```bash
cnb --help          # 顶层帮助
cnb user --help     # 子命令帮助
cnb repo list -h    # 具体操作帮助
```

## 配置优先级

1. **CLI 参数 / 环境变量** (最高)
2. **配置文件** `~/.config/cnb/auth.json`
3. **默认值** (最低)

### 环境变量

| 变量 | 说明 |
|------|------|
| `CNB_TOKEN` | API 认证令牌 |
| `CNB_API_URL` | API 地址 (默认: `https://api.cnb.cool`) |

### 配置文件

路径: `$XDG_CONFIG_HOME/cnb/auth.json` (通常为 `~/.config/cnb/auth.json`)

```json
{
  "token": "your-api-token",
  "api_url": "https://api.cnb.cool"
}
```

## 命令结构

```
cnb [OPTIONS] <COMMAND>

OPTIONS:
    --api-url <URL>    API 地址
    --token <TOKEN>    认证令牌
    -h, --help         显示帮助信息

COMMANDS:
    user    用户操作
    repo    仓库操作
    issue   Issue 操作
    pr      Pull Request 操作
    build   Build 操作

GLOBAL OPTIONS:
    --json    输出 JSON 格式 (默认: 友好文本格式)
```

### user 子命令

```
cnb user <ACTION>

ACTIONS:
    info    获取当前用户信息
```

### repo 子命令

```
cnb repo <ACTION>

ACTIONS:
    list    列出仓库
        -g, --group <SLUG>    按组织筛选
        --page <N>            指定页码
        --page-size <N>       指定分页大小
        --all                 自动翻页拉取全部
```

### issue 子命令

```
cnb issue <ACTION>

ACTIONS:
    list <owner/repo>
    get <owner/repo> <number>
    create <owner/repo> --title <title> [--body <body>]
```

### pr 子命令

```
cnb pr <ACTION>

ACTIONS:
    list <owner/repo>
    get <owner/repo> <number>
    create <owner/repo> --title <title> --source <branch> --target <branch> [--body <body>]
    merge <owner/repo> <number>
```

### build 子命令

```
cnb build <ACTION>

ACTIONS:
    list <owner/repo>
    get <owner/repo> <sn>
    trigger <owner/repo> --branch <branch> [--commit <sha>]
    cancel <owner/repo> <sn>
    logs <owner/repo> <sn>
```

## 使用示例

```bash
# 查看帮助
cnb --help
cnb user --help
cnb repo list --help

# 获取当前用户信息
cnb user info

# 列出当前用户的仓库
cnb repo list

# 列出指定组织的仓库
cnb repo list --group myorg

# 使用环境变量
CNB_TOKEN=xxx cnb user info

# 使用 CLI 参数
cnb --token xxx user info
```

## 输出格式

默认输出为友好文本格式，便于人在终端阅读；如需用于脚本/管道处理，请使用 `--json` 输出结构化 JSON。

```bash
# JSON 输出（推荐用于脚本）
cnb --json user info | jq -r '.username'
cnb --json repo list | jq -r '.[].name'

# 文本输出（默认，推荐用于交互）
cnb user info
cnb repo list
```

### 分页

大多数列表接口支持分页参数：

```bash
# 指定页码和分页大小
cnb repo list --page 1 --page-size 50
cnb issue list owner/repo --page 2 --page-size 30

# 自动分页拉取全部
cnb repo list --all
cnb pr list owner/repo --all
```

### Build

Build 相关命令中，`sn` 为构建序列号（字符串），不是数字自增 ID：

```bash
cnb build get owner/repo <sn>
cnb build logs owner/repo <sn>
cnb build cancel owner/repo <sn>
```

## 项目结构

```
src/
├── main.rs      # CLI 入口，命令定义
├── client.rs    # HTTP 客户端，API 调用
└── config.rs    # 配置文件加载
```

## 扩展指南

添加新命令：

1. 在 `main.rs` 的 `Commands` 枚举中添加子命令
2. 在 `client.rs` 中实现对应的 API 调用
3. 在 `main` 函数的 match 中处理新命令
4. **确保添加清晰的 `///` 文档注释用于 `--help` 输出**
