# Vendored FilmCraft decoders

Pure-Rust H.264 and H.265 decoders used by `rv-video`, copied from upstream so the exact code is
reviewed and pinned in this repository.

| Crate | Upstream | Commit | Copied |
|---|---|---|---|
| `filmcraft-h264` | [storytold/filmcraft](https://github.com/storytold/filmcraft) `crates/h264` | `e6d82d4eeb431f40b882e0770f8920462c475d37` | `src/`, `README.md` |
| `filmcraft-hevc` | [storytold/filmcraft](https://github.com/storytold/filmcraft) `crates/hevc` | `e6d82d4eeb431f40b882e0770f8920462c475d37` | `src/`, `README.md` |
| `filmcraft-bitstream` | [storytold/craft-libs](https://github.com/storytold/craft-libs) `crates/media/src/bitstream.rs` | `92d392cff132eaf53e13afff77f38b5c11b73aa0` | `bitstream.rs` |

The craft-libs commit is the one FilmCraft's workspace pins; upstream `filmcraft-bitstream` is a
re-export of `craft_media::bitstream`.

## Local changes

- `Cargo.toml` files rewritten with concrete values instead of FilmCraft workspace inheritance;
  `unsafe_code = "forbid"` kept from the upstream workspace lints.
- `filmcraft-bitstream/src/lib.rs` wraps the copied module (`mod bitstream; pub use bitstream::*;`).
- Not copied: upstream tests, examples, fixtures and the HEVC table-extraction script.
  The `#[cfg(test)]` modules inside `src/` need upstream test helpers and are not run here.

No source file under `src/` has been modified.

## License

MIT OR Apache-2.0, at your option. Upstream license texts and notices are in `licenses/`:
FilmCraft — Copyright (c) 2026 ArtCraft Team and the FilmCraft contributors;
craft-libs — Copyright (c) 2026 ArtCraft Team and the craft-libs contributors.

## Updating

Copy the same paths from a newer upstream commit, update the commits above, run
`cargo test --workspace` and the decode benchmark (`cargo run -p rv-video --release --example bench`).
