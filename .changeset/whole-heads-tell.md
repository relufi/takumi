---
"takumi": minor
---

**BREAKING CHANGE: Refactor public style API to declaration-based styles**

Replace the old field-based / `CssValue`-driven style construction with `StyleDeclaration` and `style.with(...)`.

Before:

````rust
let style = StyleBuilder::default()
  .font_size(Some(48.0.into()))
  .margin(Sides([Px(4.0); 4]))
  .build()
  .unwrap();
```

After:

```rust
let style = Style::default()
  .with(StyleDeclaration::font_size(Px(48.0).into()))
  .with_margin(Sides([Px(4.0); 4]));
````
