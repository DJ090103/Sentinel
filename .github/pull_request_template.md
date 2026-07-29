## What does this change?

<!-- One or two sentences. What, not why-in-detail — save that for the description below if needed. -->

## Why?

<!-- Link an issue if one exists. If this is a milestone task, name it. -->

## How was this tested?

<!-- cargo test output, manual steps, screenshots for UI changes, etc. -->

## Checklist

- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] `pnpm --filter sentinel-desktop lint` and `build` pass (if frontend touched)
- [ ] Public functions/types have doc comments
- [ ] No new `unwrap()` outside genuinely infallible cases
