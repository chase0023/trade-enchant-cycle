// HEAD helper for `Game::native_key`.
fn tec_horizon_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::tec_hooks::key(game, key, pressed);
}
