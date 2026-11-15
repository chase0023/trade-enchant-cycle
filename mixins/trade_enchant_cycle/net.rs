// HEAD helper for `Game::native_net_from_client` (API 40).
fn tec_horizon_net(
    game: &mut crate::game::Game<'_>,
    player: u128,
    channel: &str,
    data: &[u8],
) {
    crate::game::tec_hooks::net_from_client(game, player, channel, data);
}
