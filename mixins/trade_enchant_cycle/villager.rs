// Villager trade rerolling.
//
// Injected into `crates/horizon-client/src/game/trading.rs` as
// `pub(crate) mod tec_villager`, so it can use the game's own offer
// generation:
//
//   * `Game::ensure_offers`        -- `AbstractVillager.getOffers`
//   * `Game::offers_from_set`      -- `AbstractVillager.addOffersFromTradeSet`
//
// Those read the data-driven `villager_trade` / `trade_set` registries with
// the world's `RandomSequences`, so a reroll is vanilla's own roll: profession
// trade sets, level, weights, enchantment and map functions, prices, stock and
// demand all come from the same code that made the villager's first offers.
// Nothing here invents an offer, and nothing resets a trade that was used:
// `tec_core::plan_reroll` keeps every used or out-of-stock offer and fills only
// the rest of the merchant's list.

use crate::game::tec_core as core;
use crate::game::Game;
use horizon_client::mobs::{
    Species,
    render::{PROFESSIONS, VILLAGER_TYPES},
    villager,
};
use horizon_client::merchant::MerchantOffer;

/// `Villager.NONE` and `Villager.NITWIT`: no trade set.
const NO_PROFESSION: [u8; 2] = [0, 11];

/// What one offer looks like to the planner.
pub(in crate::game) fn offer_key(offer: &MerchantOffer) -> core::OfferKey {
    let mut enchantments: Vec<(String, u32)> = offer
        .result
        .enchantments()
        .iter()
        .map(|(id, level)| (id.to_string(), *level))
        .collect();
    enchantments.extend(
        offer
            .result
            .stored_enchantments()
            .iter()
            .map(|(id, level)| (id.to_string(), *level)),
    );
    core::OfferKey {
        // `MerchantOffer.outOfStock` and "taken at least once": both are
        // trades the player has already had, which a reroll leaves alone.
        locked: offer.uses > 0 || offer.out_of_stock(),
        result: offer.result.item.to_string(),
        result_count: offer.result.count,
        result_enchantments: enchantments,
        cost_a: offer.base_cost_a.stack.item.to_string(),
        cost_a_count: offer.base_cost_a.stack.count,
        cost_b: offer
            .cost_b
            .as_ref()
            .map(|cost| (cost.stack.item.to_string(), cost.stack.count)),
    }
}

/// Rolls one merchant's offers again, given what it has now.
///
/// `existing` is the live list (the mob's `Trader.offers`, or a trading
/// session's), `profession`/`level`/`variant` are the merchant's own, and
/// `wandering` picks the wandering trader's three sets instead of the
/// villager's levels. The fresh roll goes through the game's own
/// `offers_from_set` for exactly those sets, then `tec_core::plan_reroll`
/// keeps every used or out-of-stock offer and fills the rest.
///
/// Returns the outcome and, when it rerolled, the new list to install.
pub(in crate::game) fn reroll_offers(
    game: &mut Game<'_>,
    existing: Vec<MerchantOffer>,
    profession: &str,
    level: i32,
    variant: &str,
    wandering: bool,
    origin: [f64; 3],
    keep_used_trades: bool,
) -> (core::Outcome, Vec<MerchantOffer>) {
    if existing.is_empty() {
        return (core::Outcome::NothingToReroll, existing);
    }
    let existing_keys: Vec<core::OfferKey> = existing.iter().map(offer_key).collect();
    if keep_used_trades && !core::has_unlocked(&existing_keys) {
        return (core::Outcome::NothingToReroll, existing);
    }
    // The sets this merchant's offers come from at its level. A villager of
    // level N has the sets of levels 1..=N (vanilla adds the next one on each
    // level-up); a wandering trader has its three.
    let sets: Vec<String> = if wandering {
        core::WANDERING_SETS.iter().map(|s| (*s).to_string()).collect()
    } else {
        (1..=level.clamp(1, core::MAX_LEVEL))
            .filter_map(|l| core::trades_set_id(profession, l))
            .collect()
    };
    let mut fresh: Vec<MerchantOffer> = Vec::new();
    for set in &sets {
        if game.stations().trades.sets.contains_key(set) {
            fresh.extend(game.offers_from_set(set, variant, origin));
        }
    }
    let fresh_keys: Vec<core::OfferKey> = fresh.iter().map(offer_key).collect();
    let plan = core::plan_reroll(&existing_keys, &fresh_keys, keep_used_trades);
    if plan.added.is_empty() {
        return (core::Outcome::NothingToReroll, existing);
    }
    let added = plan.added.len();
    let offers = core::apply_plan(&existing, &fresh, &plan);
    (core::Outcome::Rerolled { added }, offers)
}

/// Rerolls one merchant's offers.
///
/// Guards, in order: this is the authoritative side, the entity is a villager
/// (or a wandering trader when the config allows it) this level holds, it is
/// alive, adult, not trading and not asleep, and it has a trade set. Then the
/// fresh roll is planned against the current offers by `tec_core`.
pub(in crate::game) fn reroll(
    game: &mut Game<'_>,
    entity: i32,
    kind: &str,
    include_wandering_traders: bool,
    keep_used_trades: bool,
) -> core::Outcome {
    // The server is the authority for offers; a client's copy is a prediction
    // and rerolling it would desync the trade screen.
    if game.remote.is_some() {
        return core::Outcome::UnsupportedServer;
    }
    let expected = match kind {
        "minecraft:villager" => Species::Villager,
        "minecraft:wandering_trader" => Species::WanderingTrader,
        _ => return core::Outcome::NotAVillager,
    };
    if expected == Species::WanderingTrader && !include_wandering_traders {
        return core::Outcome::NotAVillager;
    }
    let Some(index) = game.mobs.iter().position(|m| m.id == entity as u32) else {
        // A stale id: the entity was removed between the click and the hook.
        return core::Outcome::NotAVillager;
    };
    {
        let mob = &game.mobs[index];
        if mob.species() != expected || !mob.alive() {
            return core::Outcome::NotAVillager;
        }
        if mob.is_baby() {
            return core::Outcome::Baby;
        }
        // `AbstractVillager.isTrading` (the mob has a trading player).
        if mob.extra.flag2 {
            return core::Outcome::Trading;
        }
        if expected == Species::Villager {
            if villager::sleeping(mob) {
                return core::Outcome::Sleeping;
            }
            if NO_PROFESSION.contains(&mob.extra.variant2) {
                return core::Outcome::NoProfession;
            }
        }
    }

    // `AbstractVillager.getOffers`, exactly as a first trade does it.
    game.ensure_offers(index);

    let (existing, variant, profession, level, origin) = {
        let mob = &game.mobs[index];
        let trade = mob.extra.trade.as_deref();
        let existing = trade.map(|t| t.offers.clone()).unwrap_or_default();
        let variant = format!(
            "minecraft:{}",
            VILLAGER_TYPES[mob.extra.variant as usize % VILLAGER_TYPES.len()]
        );
        let profession = PROFESSIONS[mob.extra.variant2 as usize % PROFESSIONS.len()].to_string();
        let level = i32::from(mob.extra.variant3.max(1)).clamp(1, core::MAX_LEVEL);
        let origin = [mob.position.x, mob.position.y, mob.position.z];
        (existing, variant, profession, level, origin)
    };
    let (outcome, offers) = reroll_offers(
        game,
        existing,
        &profession,
        level,
        &variant,
        expected == Species::WanderingTrader,
        origin,
        keep_used_trades,
    );
    if !matches!(outcome, core::Outcome::Rerolled { .. }) {
        return outcome;
    }

    // Write the offers back on the villager, keeping everything else exactly
    // as it was: experience, level, profession, restock counters and the
    // traded-player bookkeeping all live outside `offers`.
    if let Some(trade) = game.mobs[index].extra.trade.as_deref_mut() {
        trade.offers = offers;
        trade.unset = false;
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_client::items::ItemStack;
    use horizon_client::merchant::{ItemCost, MerchantOffer};

    fn cost(item: &str, count: u32) -> ItemCost {
        ItemCost {
            stack: ItemStack::new(item, count),
        }
    }

    fn offer(result: &str, uses: i32, max_uses: i32) -> MerchantOffer {
        MerchantOffer {
            base_cost_a: cost("minecraft:emerald", 1),
            cost_b: None,
            result: ItemStack::new(result, 1),
            uses,
            max_uses,
            reward_exp: true,
            special_price_diff: 0,
            demand: 0,
            price_multiplier: 0.05,
            xp: 1,
        }
    }

    /// A used offer is locked, an untouched one is not, and a fully used one
    /// is out of stock and locked too.
    #[test]
    fn offer_keys_mark_only_used_offers_locked() {
        assert!(!offer_key(&offer("minecraft:bread", 0, 12)).locked);
        assert!(offer_key(&offer("minecraft:bread", 1, 12)).locked);
        assert!(offer_key(&offer("minecraft:bread", 12, 12)).locked);
    }

    /// Applying a plan keeps every used offer and only replaces untouched
    /// ones, with the merchant's list staying the same length.
    #[test]
    fn a_plan_keeps_used_offers_and_keeps_the_list_length() {
        let existing = [offer("minecraft:bread", 0, 12), offer("minecraft:wheat", 3, 12)];
        let fresh = [
            offer("minecraft:wheat", 0, 12),
            offer("minecraft:bread", 0, 12),
            offer("minecraft:apple", 0, 12),
        ];
        let existing_keys: Vec<_> = existing.iter().map(offer_key).collect();
        let fresh_keys: Vec<_> = fresh.iter().map(offer_key).collect();
        let plan = core::plan_reroll(&existing_keys, &fresh_keys, true);
        assert_eq!(plan.kept, vec![1]);
        // fresh[0] repeats the kept wheat entry (same stack, same price), so
        // it is skipped; fresh[1] (bread) fills the one free slot.
        assert_eq!(plan.added, vec![1]);
        let mut out: Vec<MerchantOffer> = plan.kept.iter().map(|&i| existing[i].clone()).collect();
        out.extend(plan.added.iter().map(|&i| fresh[i].clone()));
        assert_eq!(out.len(), existing.len());
        assert_eq!(out[0].result.item.as_ref(), "minecraft:wheat");
        assert_eq!(out[0].uses, 3);
        assert_eq!(out[1].result.item.as_ref(), "minecraft:bread");
        assert_eq!(out[1].uses, 0);
    }
}
