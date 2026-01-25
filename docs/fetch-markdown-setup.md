# Fetch Markdown 配置说明

## 概述

`fetch_markdown` MCP 工具使用 Cloudflare Browser Rendering REST API 将网页转换为 Markdown 格式。

## 环境变量配置

### 必需的环境变量

1. **CLOUDFLARE_ACCOUNT_ID**: 你的 Cloudflare 账户 ID
2. **CLOUDFLARE_API_TOKEN**: 具有 Browser Rendering 权限的 API Token

### 获取 Account ID

1. 登录 [Cloudflare Dashboard](https://dash.cloudflare.com)
2. 在右侧边栏可以看到你的 Account ID

### 创建 API Token

1. 访问 [API Tokens 页面](https://dash.cloudflare.com/profile/api-tokens)
2. 点击 "Create Token"
3. 选择 "Create Custom Token"
4. 配置权限：
   - Account > Browser Rendering > Edit
5. 创建并复制 Token

### 配置方式

#### 开发环境

在 `wrangler.jsonc` 中配置：

```jsonc
{
  "vars": {
    "CLOUDFLARE_ACCOUNT_ID": "your-account-id",
    "CLOUDFLARE_API_TOKEN": "your-api-token"
  }
}
```

#### 生产环境

使用 Wrangler secrets：

```bash
# 设置 Account ID
wrangler secret put CLOUDFLARE_ACCOUNT_ID

# 设置 API Token
wrangler secret put CLOUDFLARE_API_TOKEN
```

或通过 Cloudflare Dashboard：
1. Workers & Pages > 你的 Worker
2. Settings > Variables
3. 添加环境变量

## 使用示例

```typescript
// MCP 工具调用
{
  "name": "fetch_markdown",
  "arguments": {
    "url": "https://example.com",
    "waitUntil": "networkidle0"  // 可选: "load", "domcontentloaded", "networkidle0", "networkidle2"
  }
}
```

## 支持的 waitUntil 选项

- `load`: 页面加载完成
- `domcontentloaded`: DOM 内容加载完成
- `networkidle0`: 网络完全空闲（无请求）
- `networkidle2`: 网络几乎空闲（最多 2 个请求）- **默认值**

## 故障排查

### 错误: Missing required environment variables

确保已正确配置 `CLOUDFLARE_ACCOUNT_ID` 和 `CLOUDFLARE_API_TOKEN`。

### 错误: Cloudflare API request failed

1. 检查 API Token 权限是否正确
2. 确认 Account ID 是否正确
3. 检查你的账户是否启用了 Browser Rendering

## 技术细节

- 使用 Cloudflare Browser Rendering REST API
- 端点: `https://api.cloudflare.com/client/v4/accounts/{accountId}/browser-rendering/markdown`
- 不再依赖 `turndown` 库（已移除）
- 不再使用 Puppeteer binding（仅用于其他功能）

## 相关文档

- [Cloudflare Browser Rendering REST API](https://developers.cloudflare.com/browser-rendering/rest-api/markdown-endpoint/)
- [API Token 权限](https://developers.cloudflare.com/fundamentals/api/get-started/create-token/)
