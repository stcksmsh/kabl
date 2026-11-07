# Reading `.kabl` files in `git diff`

A `.kabl` file is zstd-compressed, so `git diff` shows it as binary. `kabl cat` prints the log as
JSON lines, one op per line, which git can diff as text. Docs only: the repository holds no
`.kabl` files (factory patches stay folders), but a patch repository of your own can use this.

In the patch repository's `.gitattributes`:

```
*.kabl diff=kabl
```

In your git config (per clone, or `--global`), with `kabl` on your PATH:

```
git config diff.kabl.textconv "kabl cat"
git config diff.kabl.cachetextconv true
```

`git diff`, `git log -p` and `git show` then diff the op logs. Merging is still by whole file:
two edited copies of one `.kabl` conflict, so resolve by unpacking both (`kabl unpack a.kabl`) and
choosing a side. The tools:

| Command | Does |
|---|---|
| `kabl cat FILE.kabl` | Print the log as JSON lines. |
| `kabl pack DIR [OUT.kabl]` | Folder to `.kabl` (default `DIR.kabl`). `sound.toml` is not stored. |
| `kabl unpack FILE.kabl [DIR]` | `.kabl` to a folder (default: the file name without `.kabl`). |

`pack` and `unpack` never overwrite an existing path. Every error is one line on stderr and exit
code 1.
