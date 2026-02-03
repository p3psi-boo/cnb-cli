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

所有输出为 JSON 格式，便于管道处理：

```bash
cnb user info | jq '.username'
cnb repo list | jq '.[].name'
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
