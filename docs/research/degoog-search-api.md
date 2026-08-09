# Degoog native search API research

Primary sources:

- [Degoog API reference](https://degoog-org.github.io/docs/api.html)
- [Degoog repository](https://github.com/degoog-org/degoog)
- [`SearchResponse` and `EngineTiming` source contracts](https://github.com/degoog-org/degoog/blob/main/src/shared/search-types.ts)
- [Native search implementation](https://github.com/degoog-org/degoog/blob/main/src/server/search.ts)

## Integration contract

- Native web search is `GET /api/search?q=<query>&type=web&page=1` on the Degoog instance host. Responses are JSON.
- Do not send `format=json` for the native adapter. That parameter selects Degoog's optional SearXNG-compatible response shape when the instance toggle is enabled.
- Search routes are open by default. Protected instances accept `Authorization: Bearer <api-key>` and return HTTP 401 with `{ "error": "You shall not pass!" }` when authentication is missing or invalid.
- Native results expose `title`, `url`, and `snippet`; `content` mirrors `snippet` for Open WebUI compatibility. Results can also include provider metadata such as `source`, `score`, and `sources` that `wa` does not currently need.
- The response exposes `engineTimings`. Each timing contains `name`, `time`, `resultCount`, and optional `status`, `errorReason`, and `httpStatus` fields. The server records successful runs with status `ok`; failed runs carry a classified non-OK status and optional reason/status code.
- A Degoog instance with no active engines returns a healthy empty response with `results: []` and `engineTimings: []`. This must not be treated as a degraded search.
- Degoog caps `page` at 10. The initial `wa` adapter uses page 1 and applies its own result limit after normalization and URL deduplication.
- Raw mode should return the first native response body unchanged. It therefore has a provider-dependent schema.

## Architectural implications

- Provider adapters should translate native results and upstream failure evidence into one internal response shape.
- Retry-on-degraded-empty, URL deduplication, and result limiting are provider-neutral policy and should live above adapters.
- Bearer authentication is an adapter concern. The CLI/config layer passes an optional key but must not understand request headers.
- A small internal enum is sufficient for SearXNG and Degoog and remains easy to extend with Tavily or Exa. A public plugin trait or dynamic provider registry is unnecessary until external crates need to supply adapters.

## Deliberate exclusions

The first integration does not use SSE search, `/api/search/retry`, suggestions, search tabs, engine allowlists, commands, lucky redirects, or the beta MCP sidecar.
