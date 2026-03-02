---
"@takumi-rs/wasm": minor
---

**Reverting `.asUint8Array()` changes**

As it's very dangerous to use `asUint8Array()` without proper handling and recycling, we are reverting the changes.

The `render` and `renderAnimation` methods now return `Uint8Array` instead of `WasmBuffer` class.

```tsx
const image = renderer.render(node, options);

controller.enqueue(image);
```
