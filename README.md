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

Config is ID-focused. Each non-comment line is:

```text
<id> <name>
```

Example:

```text
4 网络流行新词
```

Run:

```shell
cargo run -- --config scel2rime.conf
```

or:

```shell
scel2rime --config scel2rime.conf
```

The app downloads SCEL files with `curl`, caches them under `.scel2rime-cache/`, then writes one RIME dictionary per ID, such as:

```text
luna_pinyin.sogou.4.dict.yaml
```

Generated rows follow RIME dictionary shape used by projects like [`rime-frost`](https://github.com/gaboolic/rime-frost):

```text
词条	pin yin	weight
```
