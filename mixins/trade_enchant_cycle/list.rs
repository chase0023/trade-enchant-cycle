// HEAD helper for `Game::native_mods`: the Options > Mods row.
fn tec_horizon_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::tec_hooks::listed(mods);
}
