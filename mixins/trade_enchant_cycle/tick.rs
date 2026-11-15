// HEAD helper for `Game::native_tick`.
fn tec_horizon_tick(game: &mut crate::game::Game<'_>) {
    crate::game::tec_hooks::tick(game);
}
