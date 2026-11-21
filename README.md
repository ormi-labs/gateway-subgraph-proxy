# gateway-subgraph-proxy

A small HTTP proxy that lets [The Graph Gateway](https://github.com/edgeandnode/gateway) send subgraph queries to any GraphQL endpoint.

The Gateway expects to talk to an indexer, and indexers wrap their GraphQL results in a JSON envelope. This proxy accepts the Gateway's query, forwards it to a plain GraphQL endpoint you configure (for example a hosted subgraph or a `graph-node` query URL), and returns the result in that envelope.

```
Graph Gateway ──POST /──▶ gateway-subgraph-proxy ──POST──▶ target GraphQL endpoint
              ◀── {"graphQLResponse": "..."} ──        ◀── raw GraphQL response ──
```

## How it works

The proxy has a single route, `POST /`.

**Request.** The body must be JSON with a `query` field and an optional `variables` field:

```json
{ "query": "{ tokens(first: 5) { id } }", "variables": { "foo": "bar" } }
```

Both fields are passed through to the target unchanged: the proxy does not parse or re-encode them. Any other fields in the body are dropped.

**Forwarding.** The proxy sends `{"query": ..., "variables": ...}` as a `POST` to the target URL. If you set an auth value, it goes in the `Authorization` header exactly as given.

**Response.** If the target answers with a 2xx status, the proxy returns `200 OK` with the target's response body, as a string, in `graphQLResponse`:

```json
{ "graphQLResponse": "{\"data\":{\"tokens\":[{\"id\":\"0x...\"}]}}" }
```

GraphQL errors the target returns with a 2xx status are passed through inside `graphQLResponse`. The proxy does not add an attestation.

**Errors.**

| Situation                                                       | Proxy response                                              |
| --------------------------------------------------------------- | ----------------------------------------------------------- |
| Body is not valid JSON or has no `query` field                  | `4xx` from axum's JSON extractor                            |
| Target returns a non-2xx status, can't be reached, or times out | `502 Bad Gateway`, body `Error sending request to provider` |

The cause of a `502` is written to the logs. It is not sent back to the caller.

## Configuration

You can set each option as a command-line flag or as an environment variable.

| Flag                | Environment variable    | Default        | Description                                                                                                     |
| ------------------- | ----------------------- | -------------- | --------------------------------------------------------------------------------------------------------------- |
| `-u`, `--url`       | `PROXY_TARGET_URL`      | required       | GraphQL endpoint to forward queries to                                                                          |
| `-a`, `--auth`      | `PROXY_TARGET_AUTH`     | none           | Value sent in the `Authorization` header, e.g. `Bearer <token>`                                                 |
| `--listen`          | `PROXY_LISTEN_ADDRESS`  | `0.0.0.0:3000` | Address and port to listen on                                                                                   |
| `--request-timeout` | `PROXY_REQUEST_TIMEOUT` | `30s`          | Timeout for each request to the target, in [humantime](https://docs.rs/humantime) format (`500ms`, `30s`, `1m`) |

`--auth` is sent verbatim, so include the scheme (`Bearer `, `Basic `, and so on) if the target needs one.

Logging uses `tracing` and reads the `RUST_LOG` environment variable. The default level is `info`, which logs every HTTP request. Set `RUST_LOG=debug` for more detail.

Run `proxy --help` to see all options.

## Running

### With Cargo

You need Rust 1.97. The pinned version in [rust-toolchain.toml](rust-toolchain.toml) is installed automatically if you use `rustup`.

```sh
cargo run --release -- \
  --url https://example.com/subgraphs/name/my-subgraph \
  --auth "Bearer $TOKEN"
```

The release binary is written to `target/release/proxy`.

### With Docker

```sh
docker build -t gateway-subgraph-proxy .

docker run --rm -p 3000:3000 \
  -e PROXY_TARGET_URL=https://example.com/subgraphs/name/my-subgraph \
  -e PROXY_TARGET_AUTH="Bearer $TOKEN" \
  gateway-subgraph-proxy
```

The image builds the binary in a `rust:1.97-slim-bookworm` stage, then copies it into `debian:bookworm-slim`, where it runs as the non-root user `proxy` (UID 1000).

### Testing it

```sh
curl -s localhost:3000/ \
  -H 'Content-Type: application/json' \
  -d '{"query":"{ _meta { block { number } } }"}'
```

## Development

The whole service is in [src/main.rs](src/main.rs). It is built with [axum](https://github.com/tokio-rs/axum) and [reqwest](https://github.com/seanmonstar/reqwest) on [tokio](https://tokio.rs).

Pre-commit hooks are defined in [.pre-commit-config.yaml](.pre-commit-config.yaml). To install them:

```sh
pre-commit install
```

The hooks run `cargo fmt --check`, `cargo clippy`, `yamlfmt` and the standard file checks. They also block commits made directly to `main`, so work on a branch and open a pull request.

Release builds use `opt-level = 3`, thin LTO and one codegen unit. This makes them faster but slower to compile.

## Limitations

- There is no health-check endpoint. `POST /` is the only route.
- Each proxy instance forwards to one target URL.
- Request headers from the Gateway are not forwarded. Only `Authorization` is set, and only from `--auth`.
- The target's response body is passed through even if it is not valid JSON.
