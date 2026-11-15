// The `cargo test` half of Trade & Enchant Cycle.
//
// This library is not the mod. The mod is `mixins/trade_enchant_cycle/`,
// applied to Horizon's own source by horizon-mixins. This crate compiles the
// *same file* the game compiles as `crate::game::tec_core`, so the tests in
// `tests/` run against the shipped decisions and config reader rather than a
// copy of them.
//
// Files that need the game (`config.rs`, `hooks.rs`, `villager.rs`,
// `enchanting.rs` in `mixins/trade_enchant_cycle/`) are type-checked by the
// native build.

#[path = "../../mixins/trade_enchant_cycle/core.rs"]
mod core_impl;

pub use core_impl::*;
