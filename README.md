# Usage

`scel2rime` converts Sogou `.scel` cell dictionaries into RIME `.dict.yaml` files.

## Convert one local SCEL file

```shell
cargo run -- popular_now.scel
```

Or after installing/building:

```shell
scel2rime popular_now.scel
```

This produces:

```text
luna_pinyin.sogou.popular_now.dict.yaml
```

## Download and convert dictionaries from config

Config is ID-focused. Each non-comment line is either an ID or an ID plus name:

```text
<id>
<id> <name>
```

Examples:

```text
4
77212 高中常考古诗词【官方推荐】
```

When name is omitted, the app fetches Sogou's detail page for that ID and resolves the official download name.

Run:

```shell
cargo run -- --config scel2rime.conf
```

or:

```shell
scel2rime --config scel2rime.conf
```

The app downloads SCEL bytes with native Rust HTTP, converts them in memory, and writes one RIME dictionary per ID under `dist/`, such as:

```text
dist/luna_pinyin.sogou.4.dict.yaml
```

Generated rows follow RIME dictionary shape used by projects like [`rime-frost`](https://github.com/gaboolic/rime-frost):

```text
词条	pin yin	weight
```

## Releases

GitHub Actions builds downloadable binaries when a version tag is pushed.

```shell
git tag v0.1.0
git push origin v0.1.0
```

The release workflow creates a GitHub Release and uploads archives for:

- Linux x86_64: `x86_64-unknown-linux-gnu`
- macOS Intel: `x86_64-apple-darwin`
- macOS Apple Silicon: `aarch64-apple-darwin`
- Windows x86_64: `x86_64-pc-windows-msvc`

Each archive includes the `scel2rime` binary, README, and sample `scel2rime.conf`.
