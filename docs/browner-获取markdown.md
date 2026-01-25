---
title: /markdown - Extract Markdown from a webpage · Cloudflare Browser Rendering docs
description: The /markdown endpoint retrieves a webpage's content and converts
  it into Markdown format. You can specify a URL and optional parameters to
  refine the extraction process.
lastUpdated: 2025-12-29T09:32:47.000Z
chatbotDeprioritize: false
source_url:
  html: https://developers.cloudflare.com/browser-rendering/rest-api/markdown-endpoint/
  md: https://developers.cloudflare.com/browser-rendering/rest-api/markdown-endpoint/index.md
---

The `/markdown` endpoint retrieves a webpage's content and converts it into Markdown format. You can specify a URL and optional parameters to refine the extraction process.

## Endpoint

```txt
https://api.cloudflare.com/client/v4/accounts/<accountId>/browser-rendering/markdown
```

## Required fields

You must provide either `url` or `html`:

* `url` (string)
* `html` (string)

## Common use cases

* Normalize content for downstream processing (summaries, diffs, embeddings)
* Save articles or docs for editing or storage
* Strip styling/scripts and keep readable content + links

## Basic usage

### Convert a URL to Markdown

* curl

  This example fetches the Markdown representation of a webpage.

  ```bash
  curl -X 'POST' 'https://api.cloudflare.com/client/v4/accounts/<accountId>/browser-rendering/markdown' \
    -H 'Content-Type: application/json' \
    -H 'Authorization: Bearer <apiToken>' \
    -d '{
      "url": "https://example.com"
    }'
  ```

  ```json
    "success": true,
    "result": "# Example Domain\n\nThis domain is for use in illustrative examples in documents. You may use this domain in literature without prior coordination or asking for permission.\n\n[More information...](https://www.iana.org/domains/example)"
  }
  ```

* TypeScript SDK

  ```typescript
  import Cloudflare from "cloudflare";


  const client = new Cloudflare({
    apiToken: process.env["CLOUDFLARE_API_TOKEN"],
  });


  const markdown = await client.browserRendering.markdown.create({
    account_id: process.env["CLOUDFLARE_ACCOUNT_ID"],
    url: "https://developers.cloudflare.com/",
  });


  console.log(markdown);
  ```

### Convert raw HTML to Markdown

Instead of fetching the content by specifying the URL, you can provide raw HTML content directly.

```bash
curl -X 'POST' 'https://api.cloudflare.com/client/v4/accounts/<accountId>/browser-rendering/markdown' \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer <apiToken>' \
  -d '{
    "html": "<div>Hello World</div>"
  }'
```

```json
{
  "success": true,
  "result": "Hello World"
}
```

## Advanced usage

Looking for more parameters?

Visit the [Browser Rendering API reference](https://developers.cloudflare.com/api/resources/browser_rendering/subresources/markdown/methods/create/) for all available parameters, such as setting HTTP credentials using `authenticate`, setting `cookies`, and customizing load behavior using `gotoOptions`.

### Exclude unwanted requests (for example, CSS)

You can refine the Markdown extraction by using the `rejectRequestPattern` parameter. In this example, requests matching the given regex pattern (such as CSS files) are excluded.

```bash
curl -X 'POST' 'https://api.cloudflare.com/client/v4/accounts/<accountId>/browser-rendering/markdown' \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer <apiToken>' \
  -d '{
    "url": "https://example.com",
    "rejectRequestPattern": ["/^.*\\.(css)/"]
  }'
```

```json
{
  "success": true,
  "result": "# Example Domain\n\nThis domain is for use in illustrative examples in documents. You may use this domain in literature without prior coordination or asking for permission.\n\n[More information...](https://www.iana.org/domains/example)"
}
```

### Handling JavaScript-heavy pages

For JavaScript-heavy pages or Single Page Applications (SPAs), the default page load behavior may return empty or incomplete results. This happens because the browser considers the page loaded before JavaScript has finished rendering the content.

The simplest solution is to use the `gotoOptions.waitUntil` parameter set to `networkidle0` or `networkidle2`:

```json
{
  "url": "https://example.com",
  "gotoOptions": {
    "waitUntil": "networkidle0"
  }
}
```

For faster responses, advanced users can use `waitForSelector` to wait for a specific element instead of waiting for all network activity to stop. This requires knowing which CSS selector indicates the content you need has loaded. For more details, refer to [REST API timeouts](https://developers.cloudflare.com/browser-rendering/reference/timeouts/).

### Set a custom user agent

You can change the user agent at the page level by passing `userAgent` as a top-level parameter in the JSON body. This is useful if the target website serves different content based on the user agent.

Note

The `userAgent` parameter does not bypass bot protection. Requests from Browser Rendering will always be identified as a bot.

## Troubleshooting

If you have questions or encounter an error, see the [Browser Rendering FAQ and troubleshooting guide](https://developers.cloudflare.com/browser-rendering/faq/).
