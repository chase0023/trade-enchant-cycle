// HEAD helper for `Game::native_interact_entity` (API 29).
fn tec_horizon_interact_entity(
    game: &mut crate::game::Game<'_>,
    entity: i32,
    kind: &str,
    stack: Option<&horizon_client::items::ItemStack>,
    claimed: &mut bool,
) {
    let _ = stack;
    crate::game::tec_hooks::interact_entity(game, entity, kind, claimed);
}
