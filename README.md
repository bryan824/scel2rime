# Usage

`scel2rime` converts Sogou `.scel` cell dictionaries into RIME `.dict.yaml` files.

Example source dictionary: [Sogou 网络流行新词](https://pinyin.sogou.com/d/dict/download_cell.php?id=4&name=%E7%BD%91%E7%BB%9C%E6%B5%81%E8%A1%8C%E6%96%B0%E8%AF%8D&f=detail).

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

Generated rows follow RIME dictionary shape used by projects like [`rime-frost`](https://github.com/gaboolic/rime-frost):

```text
词条	pin yin	weight
```
