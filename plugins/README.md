# sentrux plugins

Language plugins for [sentrux](https://github.com/sentrux/sentrux) — tree-sitter grammars and structural queries.

All 23 plugins install automatically on first run. No manual setup needed.

## Available languages

| Language | Extensions |
|----------|-----------|
| Bash | `sh`, `bash` |
| C | `c`, `h` |
| C# | `cs` |
| C++ | `cpp`, `cc`, `cxx`, `hpp`, `hh`, `hxx` |
| CSS | `css` |
| Elixir | `ex`, `exs` |
| GDScript (Godot) | `gd` |
| Go | `go` |
| Haskell | `hs` |
| HTML | `html`, `htm` |
| Java | `java` |
| JavaScript | `js`, `mjs`, `cjs`, `jsx` |
| Lua | `lua` |
| PHP | `php` |
| Python | `py` |
| R | `r`, `R` |
| Ruby | `rb` |
| Rust | `rs` |
| Scala | `scala`, `sc` |
| SCSS | `scss` |
| Swift | `swift` |
| TypeScript | `ts`, `mts`, `cts`, `tsx` |
| Zig | `zig` |

## Manual plugin management

```bash
sentrux plugin list              # show installed plugins
sentrux plugin add <name>        # install a single plugin
sentrux plugin remove <name>     # remove a plugin
sentrux plugin add-standard      # reinstall all 23 standard plugins
```

## Create a new plugin

```bash
# 1. Init template
sentrux plugin init my-language

# 2. Build the tree-sitter grammar
cd ~/.sentrux/plugins/my-language/grammar-src
tree-sitter generate
cc -shared -fPIC -o ../grammars/darwin-arm64.dylib src/parser.c  # macOS
cc -shared -fPIC -o ../grammars/linux-x86_64.so src/parser.c     # Linux

# 3. Write queries/tags.scm

# 4. Validate
sentrux plugin validate ~/.sentrux/plugins/my-language
```

## Plugin structure

```
<language>/
├── plugin.toml          # manifest (name, extensions, capabilities, checksums)
├── queries/
│   └── tags.scm         # tree-sitter queries for structural extraction
└── grammars/            # built by CI for each platform
    ├── darwin-arm64.dylib
    └── linux-x86_64.so
```

### Query captures

| Capability | Captures | Description |
|-----------|---------|-------------|
| functions | `@func.def`, `@func.name` | Function/method definitions |
| classes | `@class.def`, `@class.name` | Class/struct/type definitions |
| imports | `@import` or `@import.module` | Import/require/use statements |
| calls | `@reference.call` | Function call sites |

## License

MIT
