// Merchant rerolling from the trading screen.
//
// Injected into `crates/horizon-client/src/game/lan_trading.rs` as
// `pub(in crate::game) mod tec_merchant`, next to the guest trading code it
// uses:
//
//   * the guest's open `GuestMerchant` (its live `merchant::Session`)
//   * `Game::lan_send_offers` -- `ServerPlayer.sendMerchantOffers`
//
// The offers of an open trade screen live in the session, not on the mob (the
// session is written back when the screen closes), so a reroll here changes
// the session's list and sends it to the client. Single player is a guest of
// its own integrated server, so this one path serves both.
//
// The roll itself is `tec_villager::reroll_offers`: the game's own
// `offers_from_set` for the merchant's profession and level, planned by
// `tec_core` so used and sold-out offers are never reset.

use crate::game::Game;
use crate::game::lan::LanHost;
use crate::game::tec_core as core;
use crate::game::trading::tec_villager;

/// Rerolls the offers of the merchant this player has open.
///
/// Guards: the authoritative side, the player's own open trading screen (a
/// request names no merchant, so it can only reach the requester's), and the
/// villager rules `reroll_offers` applies.
pub(in crate::game) fn reroll(
    game: &mut Game<'_>,
    player: u128,
    keep_used_trades: bool,
) -> core::Outcome {
    if game.remote.is_some() {
        return core::Outcome::UnsupportedServer;
    }
    let Some(mut host) = game.lan.take() else {
        return core::Outcome::NoMenu;
    };
    let conn = host
        .hub
        .guests
        .iter()
        .find(|g| g.uuid == player)
        .map(|g| g.conn);
    let outcome = match conn {
        Some(conn) => reroll_guest(game, &mut host, conn, keep_used_trades),
        None => core::Outcome::NoMenu,
    };
    game.lan = Some(host);
    outcome
}

/// The guest's own trading screen, rerolled and sent back.
fn reroll_guest(
    game: &mut Game<'_>,
    host: &mut LanHost,
    conn: u32,
    keep_used_trades: bool,
) -> core::Outcome {
    // Taken out of the guest's play state so the game can be borrowed for the
    // roll; put back before returning, whatever happened.
    let Some(mut gm) = host.play.get_mut(&conn).and_then(|p| p.merchant.take()) else {
        // No guest window (the deprecated in-frame path, or a screen this
        // half opened itself): reroll its own station instead.
        return reroll_own_station(game);
    };
    let (existing, profession, variant, level, wandering, origin) = {
        let session = &gm.merchant.session;
        (
            session.offers.clone(),
            session.profession.clone(),
            session.variant.clone(),
            session.level,
            !session.villager,
            session.origin,
        )
    };
    let (outcome, offers) = tec_villager::reroll_offers(
        game,
        existing,
        &profession,
        level,
        &variant,
        wandering,
        origin,
        keep_used_trades,
    );
    if matches!(outcome, core::Outcome::Rerolled { .. }) {
        gm.merchant.session.offers = offers;
        // The client's list is the server's: send the new offers, and with
        // them the level, experience and price data it draws.
        game.lan_send_offers(host, conn, &mut gm, true);
    }
    host.play.entry(conn).or_default().merchant = Some(gm);
    outcome
}

/// The host's own trading screen: `Game::open_merchant_state`'s station, used
/// by the deprecated in-frame path and by a menu opened without a mob. No
/// packet is needed because the server half and the screen are the same game.
fn reroll_own_station(game: &mut Game<'_>) -> core::Outcome {
    let Some(mut station) = game.station.take() else {
        return core::Outcome::NoMenu;
    };
    let Some(mut state) = station.merchant.take() else {
        game.station = Some(station);
        return core::Outcome::NoMenu;
    };
    let (existing, profession, variant, level, wandering, origin) = {
        let session = &state.session;
        (
            session.offers.clone(),
            session.profession.clone(),
            session.variant.clone(),
            session.level,
            !session.villager,
            session.origin,
        )
    };
    let (outcome, offers) = tec_villager::reroll_offers(
        game,
        existing,
        &profession,
        level,
        &variant,
        wandering,
        origin,
        true,
    );
    if matches!(outcome, core::Outcome::Rerolled { .. }) {
        state.session.offers = offers;
    }
    station.merchant = Some(state);
    game.station = Some(station);
    outcome
}
