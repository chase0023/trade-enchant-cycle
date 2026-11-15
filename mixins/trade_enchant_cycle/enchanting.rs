// Enchanting table offer rerolling.
//
// Injected into `crates/horizon-client/src/game/lan_stations.rs` as
// `pub(crate) mod tec_enchanting`, so it can reach a guest's open workstation
// menu (`GuestStation`) and the game's own offer code.
//
// The offers a table shows are vanilla's `EnchantmentMenu`: a per-player
// `xp_seed` decides the three costs and their clues, and vanilla re-seeds it
// after every enchantment (`Player.onEnchantmentPerformed`). A reroll takes the
// next seed from the world's own random and asks the game to fill the menu's
// data slots again -- the same call an enchantment makes. Nothing else is
// touched, so no lapis, no level, no item and no enchantment is spent: the
// next real click still uses the normal path.
//
// Single player runs the world on its own server thread, so the window's
// player is a guest of that server too; one code path serves single player,
// a LAN host and every LAN guest.

use super::Game;
use crate::game::lan::LanHost;
use crate::game::stations::Kind as StationKind;
use crate::game::tec_core as core;
use horizon_client::items::ItemStack;
use horizon_core::random::RandomSource;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// The last reroll's tick, per player.
///
/// There is no cooldown: the only rule is one reroll per player per tick, so a
/// packet that arrives twice in the same tick cannot re-seed the menu twice.
/// Two requests in different ticks both run.
fn same_tick(player: u128, now: u64) -> bool {
    static LAST: OnceLock<Mutex<HashMap<u128, u64>>> = OnceLock::new();
    let map = LAST.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut map) = map.lock() else {
        return false;
    };
    let previous = map.get(&player).copied().unwrap_or(0);
    if !core::cooldown_ready(now, previous, 1) {
        return false;
    }
    map.insert(player, now);
    true
}

/// `ItemStack.isEnchantable`: an enchantable item with no enchantments yet --
/// the same test `EnchantmentMenu.slotsChanged` makes.
fn enchantable(game: &Game<'_>, stack: Option<&ItemStack>) -> bool {
    stack.is_some_and(|stack| {
        game.stations().enchantments.enchantable(&stack.item) > 0 && stack.enchantments().is_empty()
    })
}

/// Rerolls `player`'s open enchanting table.
pub(in crate::game) fn reroll(game: &mut Game<'_>, player: u128) -> core::Outcome {
    if game.remote.is_some() {
        return core::Outcome::UnsupportedServer;
    }
    if !same_tick(player, game.ticks) {
        return core::Outcome::Cooldown;
    }
    // The hub is out of the game while a guest's packet runs, and this handler
    // runs with it back (`native_net_from_client`), so taking it here is safe.
    let Some(mut host) = game.lan.take() else {
        return core::Outcome::NoMenu;
    };
    let outcome = reroll_with_hub(game, &mut host, player);
    game.lan = Some(host);
    outcome
}

fn reroll_with_hub(game: &mut Game<'_>, host: &mut LanHost, player: u128) -> core::Outcome {
    let Some(gi) = host.hub.guests.iter().position(|g| g.uuid == player) else {
        return core::Outcome::NoMenu;
    };
    let conn = host.hub.guests[gi].conn;
    // The guest's open station, out of the hub while it is changed (the
    // pattern `lan_station_op` uses).
    let Some(mut station) = host.play.get_mut(&conn).and_then(|play| play.station.take()) else {
        return core::Outcome::NoMenu;
    };
    if station.kind != StationKind::Enchanting {
        host.play.entry(conn).or_default().station = Some(station);
        return core::Outcome::NoMenu;
    }
    // Only an item vanilla would offer enchantments for: a table with nothing
    // in it, or with an enchanted item, has no offers to refresh.
    if !enchantable(game, station.menu.slot(0)) {
        host.play.entry(conn).or_default().station = Some(station);
        return core::Outcome::NotEnchantable;
    }

    // The next seed, from the world's random exactly as
    // `Player.onEnchantmentPerformed` does it. The table's costs, clues and
    // the enchantment the next click applies all come from this seed.
    let seed = game.world.random.next_int();
    host.hub.guests[gi].xp_seed = seed;
    station.menu.data[3] = seed;
    // `EnchantmentMenu.slotsChanged` for the new seed: the game's own offer
    // code, with the bookshelves around this table and the item in slot 0.
    game.lan_enchant_offers(host, gi, &mut station);
    station.menu.enchant_changed = false;
    host.play.entry(conn).or_default().station = Some(station);
    // Slots, carried stack and the data slots the client shows.
    game.lan_send_station(host, conn);
    core::Outcome::EnchantingRerolled
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_client::enchant::Enchantments;
    use horizon_client::enchant::tests::{sample, sample_items};
    use horizon_client::items::Items;
    use horizon_client::stations::{apply_enchantment, enchant_offers, enchantment_list};

    /// The fixture registry plus the items these tests need an
    /// `minecraft:enchantable` value and a tag for.
    fn fixture(items_needed: &[(&str, u32)]) -> (Enchantments, Items) {
        let mut enchantments = sample();
        let mut items = sample_items();
        for (item, value) in items_needed {
            enchantments.props.insert(
                std::sync::Arc::from(*item),
                horizon_client::enchant::ItemProps {
                    enchantable: *value,
                    ..Default::default()
                },
            );
            items
                .item_tags
                .entry("minecraft:enchantable/durability".into())
                .or_default()
                .insert((*item).to_owned());
        }
        (enchantments, items)
    }

    /// Every offer is what vanilla would show: a positive cost, a clue taken
    /// from the item's own list, and an enchantment that list applies.
    fn assert_valid(enchantments: &Enchantments, items: &Items, seed: i32, item: &ItemStack) {
        let offers = enchant_offers(enchantments, items, seed, 15, Some(item));
        assert!(
            offers.costs.iter().any(|cost| *cost > 0),
            "no offer for {}",
            item.item
        );
        for slot in 0..3 {
            if offers.costs[slot] <= 0 {
                assert!(offers.clues[slot].is_none());
                continue;
            }
            let list = enchantment_list(enchantments, items, seed, slot, offers.costs[slot], &item.item);
            assert!(!list.is_empty());
            let clue = offers.clues[slot].as_ref().expect("a clue for an offered slot");
            assert!(list.contains(clue));
            let applied =
                apply_enchantment(enchantments, items, seed, slot, offers.costs[slot], item)
                    .expect("an enchantable item gets its list");
            let expected: Vec<(String, u32)> = list
                .iter()
                .map(|(id, level)| (id.to_string(), *level))
                .collect();
            let got: Vec<(String, u32)> = if &*item.item == "minecraft:book" {
                applied
                    .stored_enchantments()
                    .iter()
                    .map(|(id, level)| (id.to_string(), *level))
                    .collect()
            } else {
                applied
                    .enchantments()
                    .iter()
                    .map(|(id, level)| (id.to_string(), *level))
                    .collect()
            };
            assert_eq!(got, expected, "{} slot {slot}", item.item);
        }
    }

    /// Diamond sword, pickaxe, chestplate, bow and book all get valid offers
    /// from the game's own generator, and the book becomes an enchanted book.
    #[test]
    fn vanilla_offers_are_valid_for_every_common_enchantable() {
        let (enchantments, items) = fixture(&[
            ("minecraft:diamond_pickaxe", 10),
            ("minecraft:diamond_chestplate", 10),
            ("minecraft:bow", 1),
        ]);
        for item in [
            "minecraft:diamond_sword",
            "minecraft:diamond_pickaxe",
            "minecraft:diamond_chestplate",
            "minecraft:bow",
            "minecraft:book",
        ] {
            let stack = ItemStack::new(item, 1);
            assert_valid(&enchantments, &items, 4242, &stack);
        }
        let book = ItemStack::new("minecraft:book", 1);
        let offers = enchant_offers(&enchantments, &items, 7, 15, Some(&book));
        let slot = (0..3)
            .find(|slot| offers.costs[*slot] > 0)
            .expect("a book is offered");
        let out = apply_enchantment(&enchantments, &items, 7, slot, offers.costs[slot], &book)
            .expect("the book is enchanted");
        assert_eq!(&*out.item, "minecraft:enchanted_book");
        assert!(!out.stored_enchantments().is_empty());
    }

    /// The table offers nothing for a stack it cannot enchant, which is what
    /// the reroll refuses too (an item that already has enchantments).
    #[test]
    fn an_enchanted_or_plain_item_gets_no_offers() {
        let (enchantments, items) = fixture(&[]);
        let mut sword = ItemStack::new("minecraft:diamond_sword", 1);
        sword.enchant("minecraft:sharpness", 2);
        let offers = enchant_offers(&enchantments, &items, 99, 15, Some(&sword));
        assert_eq!(offers.costs, [0, 0, 0]);
        assert!(offers.clues.iter().all(Option::is_none));
        let offers = enchant_offers(
            &enchantments,
            &items,
            99,
            15,
            Some(&ItemStack::new("minecraft:diamond_pickaxe", 1)),
        );
        assert_eq!(offers.costs, [0, 0, 0], "no enchantability, no offers");
    }

    /// Consecutive rerolls (fresh seeds) keep producing valid offers, and two
    /// seeds really do give different ones -- a reroll is worth doing.
    #[test]
    fn consecutive_rerolls_stay_valid_and_can_change_the_offers() {
        let (enchantments, items) = fixture(&[]);
        let sword = ItemStack::new("minecraft:diamond_sword", 1);
        let mut seen: Vec<[i32; 3]> = Vec::new();
        for seed in 1000..1008 {
            assert_valid(&enchantments, &items, seed, &sword);
            let offers = enchant_offers(&enchantments, &items, seed, 15, Some(&sword));
            seen.push(offers.costs);
        }
        assert!(
            seen.windows(2).any(|pair| pair[0] != pair[1]),
            "eight seeds should not all show the same costs: {seen:?}"
        );
    }

    /// The offers depend only on the seed and the item, and computing them
    /// leaves the item exactly as it was: the reroll itself cannot spend
    /// lapis, levels, durability or the stack (it only reads them).
    #[test]
    fn a_reroll_reads_the_item_and_never_changes_it() {
        let (enchantments, items) = fixture(&[]);
        let sword = ItemStack::new("minecraft:diamond_sword", 1);
        let before = sword.clone();
        let first = enchant_offers(&enchantments, &items, 555, 15, Some(&sword));
        let again = enchant_offers(&enchantments, &items, 555, 15, Some(&sword));
        assert_eq!(first, again);
        assert_eq!(sword, before);
        let after = enchant_offers(&enchantments, &items, 556, 15, Some(&sword));
        assert_eq!(sword, before);
        // The plan type a reroll uses carries no cost at all; the only thing
        // that changes for the next click is the seed.
        assert!(after.costs.iter().all(|cost| *cost >= 0));
        assert_eq!(core::plan_reroll(&[], &[], true), core::RerollPlan::default());
    }
}
