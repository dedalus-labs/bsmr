# Vite Integration

Bessemer supports Vite-built packages through the `typescript_vite` rule, enabling modern frontend tooling without hand-written Starlark.

## Quick Start

### 1. Install Vite in your package

```json
// package.json
{
  "name": "@myapp/web",
  "type": "module",
  "devDependencies": {
    "vite": "^6.0.0"
  }
}
```

### 2. Create a Vite configuration

```typescript
// vite.config.ts
import { defineConfig } from "vite";

export default defineConfig({
  build: {
    outDir: "dist",
  },
});
```

### 3. Define the build target

```python
# BUILD.bsmr
load("@prelude//typescript:defs.bzl", "typescript_sources", "typescript_vite")

typescript_sources(
    name = "sources",
    srcs = {
        "packages/web/package.json": ":package.json",
        "packages/web/index.html": ":index.html",
        "packages/web/src/main.tsx": "src/main.tsx",
        "packages/web/vite.config.ts": ":vite.config.ts",
    },
)

typescript_vite(
    name = "app",
    config = "vite.config.ts",  # Default
    install = "//root:pnpm_install",
    package_root = "packages/web",
    sources = ":sources",
)
```

### 4. Build your application

```bash
bsmr build //packages/web:app
```

## Advanced Configuration

### Custom Configuration File

Override the default `vite.config.ts`:

```python
typescript_vite(
    name = "app",
    config = "vite.production.ts",
    install = "//root:pnpm_install",
    package_root = "packages/web",
    sources = ":sources",
)
```

### Multiple Build Targets

Build for different environments:

```python
typescript_vite(
    name = "app-dev",
    config = "vite.dev.ts",
    install = "//root:pnpm_install",
    package_root = "packages/web",
    sources = ":sources",
)

typescript_vite(
    name = "app-prod",
    config = "vite.prod.ts",
    install = "//root:pnpm_install",
    package_root = "packages/web",
    sources = ":sources",
)
```

### Workspace Root Package

For packages at the workspace root:

```python
typescript_vite(
    name = "app",
    config = "vite.config.ts",
    install = "//root:pnpm_install",
    package_root = ".",  # Workspace root
    sources = ":sources",
)
```

## Common Use Cases

### React Application

```typescript
// vite.config.ts
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: "dist",
  },
});
```

### React Router (Framework Mode)

```typescript
// vite.config.ts
import { reactRouter } from "@react-router/dev/vite";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [reactRouter()],
});
```

### Vue Application

```typescript
// vite.config.ts
import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [vue()],
});
```

### Library Mode

Build a reusable library:

```typescript
// vite.config.ts
import { defineConfig } from "vite";
import { resolve } from "path";

export default defineConfig({
  build: {
    lib: {
      entry: resolve(__dirname, "src/index.ts"),
      name: "MyLib",
      fileName: "my-lib",
    },
  },
});
```

## Hermetic Build Guarantees

Bessemer's Vite integration provides the same hermeticity guarantees as other TypeScript actions:

1. **Exact pnpm workspace:** Uses the frozen `pnpm-lock.yaml` without network access
2. **Package-local Vite:** Runs the exact locked Vite version from `node_modules`
3. **Declared sources:** Only accesses sources explicitly declared in the build graph
4. **Content-addressed cache:** Build outputs are cached by input content hash
5. **Reproducible:** Same inputs always produce the same output

## Troubleshooting

### Vite not found

```
Error: package-local tool 'vite' is unavailable
```

**Solution:** Ensure Vite is declared in `package.json` dependencies and present in the frozen install.

### Empty build output

```
Error: vite produced empty output
```

**Solution:** Check your Vite configuration's `outDir` setting. Vite must produce at least one file.

### Missing source files

```
Error: ENOENT: no such file or directory
```

**Solution:** Verify all required sources are declared in `typescript_sources` `srcs`.

### Plugin errors

Ensure all Vite plugins are installed and declared in `package.json`.

## Comparison with tsdown

| Feature | `typescript_library` (tsdown) | `typescript_vite` |
|---------|-------------------------------|-------------------|
| Use case | Library bundling | Application building |
| Output | ESM/CJS bundles | Static assets (HTML, JS, CSS) |
| Plugins | TypeScript-focused | Full Vite ecosystem |
| HMR | No | No (build-only) |
| Asset handling | Limited | Comprehensive |

## Migration from Manual Vite

Before:
```python
# Handwritten Starlark rule
genrule(
    name = "app",
    srcs = glob(["**/*"]),
    cmd = "cd $SRCDIR && npm run build && cp -r dist $OUT",
    out = "dist",
)
```

After:
```python
typescript_vite(
    name = "app",
    install = "//root:pnpm_install",
    package_root = "packages/web",
    sources = ":sources",
)
```

Benefits:
- Cache-separable by content hash
- No network access during build
- Exact pnpm workspace reconstruction
- Proper dependency tracking

## See Also

- [TypeScript Overview](./index.md)
- [tsdown Library Mode](./library.md)
- [pnpm Toolchain](../../toolchains/pnpm.md)
