# Trade & Enchant Cycle — native Rust mod for Horizon

Two quality-of-life features for Minecraft's trading and enchanting, as a
**Horizon native mod** written in Rust:

* **Villager trade cycling** — sneak + right-click a villager to reroll its
  offers. Used and sold-out trades are never reset, the profession, level and
  experience stay, and the new offers come from vanilla's own trade sets.
* **Enchanting table cycling** — a **Reroll [R]** bar under the enchanting
  panel, or the **R** key, gives the table's three offers a new seed. No lapis,
  no experience, no item, no automatic enchantment.
* Both screens (villager and enchanting) show the same **Reroll [R]** bar,
  attached to the outside of the panel's bottom edge where vanilla draws
  nothing, and both answer **R**. The bar stays put under the pointer: an item
  tooltip that reaches down to it is drawn over it, as vanilla draws tooltips
  over any widget.

This is **not** a Fabric, Forge, NeoForge, Bukkit or Spigot plugin, and it is
not a Horizon WebAssembly content mod. Horizon is Minecraft: Java Edition 26.3
rewritten in Rust; its native mods are **source-mixin bundles** — a
`horizon-mixin.toml` and Rust payloads that the launcher applies to Horizon's
own source and compiles into the game. Java mod jars do not run there.

* Version **1.1.0** — the UI fix release: the bar is a panel footer instead of
  an overlay inside the screen, **R** works with the default config, tooltips
  are never covered, and a bar click no longer throws the carried stack (§9).
* Built, run and verified in game on **Horizon Nightly
  `nightly-2026-11-01-c9d474d`** — commit
  `c9d474dd9116e84c243f8992d75c11314df56f06`, Minecraft Java **26.3**,
  `NATIVE_HOOKS_API = 80`, horizon-mixins **0.5.0**
  (`0d6b103c70633d8fd9443d8d0acd3aadaacedb06`): both features were used on a
  real world and did what they promise (§9).
* Licence: MIT. No Minecraft code, assets or data are included.

---

## 1. How a Horizon native mod is built

There is no `cargo build` that turns this repository into a loadable mod. The
repository gives you both halves:

| Part | What it is | How it is built |
|---|---|---|
| `mixins/trade_enchant_cycle/` | The native mod: `horizon-mixin.toml` + `.rs` payloads | The Horizon launcher (or `horizon-mixins`), against the pinned Horizon source |
| `Cargo.toml`, `core/`, `tests/` | The same pure decisions and config code, compiled without the game | `cargo test` on any stable Rust ≥ 1.70 |

`core/src/lib.rs` includes `mixins/trade_enchant_cycle/core.rs` by `#[path]`, so the unit tests
exercise the exact file the game compiles as `crate::game::tec_core` — not a
copy. The four game-facing payloads (`config.rs`, `hooks.rs`, `villager.rs`,
`enchanting.rs`, `merchant.rs`) need Horizon and are type-checked by the native
build.

---

## 2. Feature 1 — villager trade cycling

### What happens on a reroll

1. A player **sneak + right-clicks** a villager.
2. The mod asks the game for the villager's offers (`AbstractVillager.getOffers`,
   as a first trade does), then rolls the villager's trade sets for its level
   again through the game's own `addOffersFromTradeSet`.
3. Offers the player has **already taken** (`uses > 0`) or that are **sold out**
   (`uses >= maxUses`) are kept byte-for-byte. Only the untouched offers are
   replaced, so the merchant keeps the same number of offers it had.
4. A fresh offer that repeats a kept one (same result stack, enchantments and
   price) is dropped, and fresh offers are deduplicated among themselves.
5. The villager's profession, level, experience, restock counters and demand
   are untouched. The click is claimed, so the trade screen does not also open.

Because the roll goes through the data-driven `villager_trade` / `trade_set`
registries and the world's `RandomSequences`, it is a **vanilla roll**: trade
weights, enchanted-book levels, exploration maps, `max_uses`, `xp`, price
multipliers, demand and the villager-type predicate all behave as they do when
the game makes a villager's first offers. There is no separate trade table in
this project to drift from vanilla.

### Rules it respects

| Rule | How |
|---|---|
| Only refreshable trades reroll | Used (`uses > 0`) and sold-out trades are locked and kept |
| Never reset completed trades | Same rule; `keep_used_trades: false` opts out explicitly |
| Profession and progression | The villager's own `profession`/`level` choose the trade sets (`minecraft:<profession>/level_<n>`, levels 1..=5) |
| No impossible trades | Every offer comes from the game's own generator for that profession, level and villager type |
| Pricing and demand | Kept offers keep their price, demand and stock; fresh offers start at vanilla's values |
| No duplicated valuables | Kept entries are never duplicated; fresh entries are deduplicated by result stack + enchantments + price |
| Unemployed villagers | A villager with no profession (or a nitwit) is refused with a message |
| Babies, sleepers, trading villagers | Refused with a message; normal interaction continues |

### Activation

Default: **sneak + right-click** (`activation: "sneak_right_click"`). A plain
right-click still opens the trade screen, so trading is never in the way.

* `require_confirmation: true` makes the first sneak-right-click ask for a
  second one within 5 seconds (100 ticks). The first click is claimed, so the
  trade screen does not open under the prompt.
* `activation: "right_click"` is available but takes normal trading away from
  villagers; it is off by default for that reason.
* `include_wandering_traders: false` by default: a wandering trader's three
  sets (`buying`, `uncommon`, `common`) can be rerolled when it is turned on.
* **No cooldown**: sneak-right-click as often as you like. The only rule is one
  reroll per villager per tick, because one click reaches the server twice (main
  hand and off hand) and only the first may act.
* **In the trade screen** the same reroll is a **Reroll [R]** bar attached to
  the outside of the panel's bottom edge, as wide as the panel: the row under
  the trade list, the payment slots, the result slot and the player's hotbar
  row is empty in vanilla, so nothing of the screen's own is under it. It is
  drawn in the panel's own style (black outline, lit top-left edge, shaded
  bottom-right one, grey face) and brightens under the pointer. Clicking it, or
  pressing **R**, rerolls that merchant's offers in place: the list is
  replaced, used and sold-out trades keep their uses and prices, the level
  bar, experience and profession stay. A request names no merchant, so it can
  only reach the trader whose screen the player has open; `screen_button`
  turns the bar off and `key` rebinds the shortcut.

---

## 3. Feature 2 — enchanting table cycling

### What happens on a reroll

Vanilla's `EnchantmentMenu` shows three offers derived from one per-player
`xp_seed`, the bookshelves around the table and the item in the slot; the game
re-seeds that value after every real enchantment. A reroll takes the **next
seed from the world's own random** (the same call `Player.onEnchantmentPerformed`
makes) and asks the game's own `enchant_offers` to fill the menu's costs and
clues again.

Nothing else is touched:

* **No lapis**, **no experience levels**, **no item durability**, **no
  enchantment** is applied.
* Only an **enchantable item with no enchantments yet** gets offers, exactly
  as `ItemStack.isEnchantable` requires.
* The next real click still goes through vanilla's `clickMenuButton` path with
  the current seed, so the enchantment applied is the one the refreshed offer
  promised — the clues, costs and applied list agree.

The generation is Horizon's own port of `EnchantmentMenu.getEnchantmentList` /
`slotsChanged` (`horizon_client::stations::enchant_offers`): the
`minecraft:in_enchanting_table` enchantment tag, enchantment weights, item
enchantability, level cost ranges, compatibility (`exclusive_set`), the book's
one-enchantment trim and the clue pick all come from it. This project does not
pick random enchantments from a list.

### Activation

Default: **the Reroll bar and the R key**, both of them
(`activation: "interface_button"`, `show_button: true`).

**The bar** is a footer on the outside of the panel's bottom edge, as wide as
the panel less a 3 px inset, 15 px high. Vanilla draws nothing there in either
screen — the enchanting panel ends its third offer at y=71 and its hotbar row
at y=160, the merchant panel its own hotbar row at y=160, and then only the
border — so the bar cannot cover an offer, a slot, the "Inventory" label or the
inventory grid. It is placed from the panel's own origin and size
(`NativeScreenView.rect`), never from the window's, so it follows the panel at
every GUI scale and window size; the panel is centred with at least 37 px of
window above and below it at the smallest GUI the game allows, so the footer is
never clipped either.

It is drawn like the game's own widgets: a black outline, a lit top-left edge,
a shaded bottom-right one, a grey face that brightens under the pointer, and
the label **Reroll [R]**. A click on it is cancelled before the screen sees it,
and the release that ends that press is cancelled too, so the click cannot move
an item, cannot press a vanilla enchantment button, and cannot throw the stack
the player is carrying (a container screen reads a release outside its panel as
`THROW`).

**The key** does the same thing and sends the same request, so the two can never
diverge:

* It is bound whatever `activation` says, because the bar's label names it: a
  bar that promised `[R]` while R did nothing was the bug this fixes. What
  `activation` still decides is whether the bar is drawn (`"key"` leaves only
  the shortcut).
* It is read from the **open screen's own key hook**
  (`native_screen_key_pressed_pre`), which fires only while a screen is open,
  says which screen it is, and says whether the key is the press or the key
  repeating while held. So: one press is one reroll however long R is held, the
  shortcut works with the pointer over an item (its tooltip does not matter),
  and the inventory, the pause screen and world gameplay answer nothing.
* Default `key: "key.keyboard.r"`; `"r"` is accepted too. The bar's label shows
  the bound letter.

**The bar never disappears, and tooltips stay on top.** The game draws its
tooltips *before* a native mod's `native_screen_render_post`, so a bar drawn
there would be painted over one. The bar is always drawn (hovering an item does
not hide it); when the hovered item's tooltip actually reaches down to the
bar's strip — an item in the panel's lowest row, or a tall tooltip — the
tooltip is drawn again over the bar, with the game's own lines
(`stack_tooltip`), geometry and box/text drawing (`draw_tooltip_lines`) at the
same position. That is the order vanilla uses: the tooltip is the topmost thing
on a screen and the widget under it stays visible around it. Only tooltips that
reach the bar are drawn again, so nothing is duplicated elsewhere.

**Under the hood:** the request travels from the window to the server half over
the mod channel `trade_enchant_cycle:reroll` (Horizon API 40), because the
**server owns the seed and the menu data**. The client never rewrites the offers
itself. There is no cooldown: every press and every click is sent, and a click
is matched in the
screen's own panel coordinates against two readings of the pointer — where the
panel was drawn last frame and where this event's own screen says it is —
because the game's drawing and its input can disagree about the panel's origin
after a window resize or a DPI change.

---

## 4. Configuration

The first load writes `config/trade-enchant-cycle.json` beside `options.txt`
(Windows: `%APPDATA%\horizon\config\trade-enchant-cycle.json`; a custom
instance: that instance's game folder). Horizon API 26
(`Game::native_config_dir`) makes the folder.

```json
{
  "villager_trade_cycle": {
    "enabled": true,
    "activation": "sneak_right_click",
    "require_confirmation": false,
    "keep_used_trades": true,
    "include_wandering_traders": false,
    "screen_button": true,
    "key": "key.keyboard.r"
  },
  "enchanting_cycle": {
    "enabled": true,
    "activation": "interface_button",
    "free_rerolls": true,
    "reroll_cost": 0,
    "show_button": true,
    "key": "key.keyboard.r"
  },
  "notifications": {
    "enabled": true,
    "show_success": true,
    "show_errors": true
  }
}
```

| Setting | Values | Meaning |
|---|---|---|
| `villager_trade_cycle.enabled` | bool | The villager feature |
| `…activation` | `sneak_right_click`, `right_click` | How a reroll starts |
| `…require_confirmation` | bool | Two sneak-right-clicks within 100 ticks |
| `…keep_used_trades` | bool | Keep used/sold-out offers (default). `false` replaces the whole list |
| `…include_wandering_traders` | bool | Also reroll wandering traders |
| `…screen_button` | bool | Draw the Reroll bar in the trading screen |
| `…key` | key name | `key.keyboard.r`, or just `r`: the trading screen's shortcut |
| `enchanting_cycle.enabled` | bool | The enchanting feature |
| `…activation` | `interface_button`, `key`, `both` | Whether the bar is drawn. The **R** shortcut answers in the table's screen whatever this says (the bar's label names it) |
| `…free_rerolls`, `…reroll_cost` | bool, 0 | Must stay `true` / `0`. A config that asks for a cost is **refused** (the feature promises free rerolls), and the reason is logged |
| `…show_button` | bool | Draw the Reroll button |
| `…key` | key name | `key.keyboard.r`, or just `r` |
| `notifications.*` | bool | Action-bar feedback, and which kinds |

Notes:

* The file is read once per process. **Options → Mods → Trade & Enchant Cycle →
  Reload config file** re-reads it; the same page toggles the two features and
  notifications and writes the file back.
* A missing key keeps its default, a wrong type is ignored field by field, and
  unreadable JSON falls back to the defaults with one log line — a half-edited
  file never stops the game.
* The player's own on/off switch in **Options → Mods** still applies on top of
  the config (`Game::native_on`).
* `cooldown_ticks` is **gone**: rerolls have no cooldown. A file written by
  1.1.0 still carries the key; it is ignored and simply not written again.

---

## 5. Repository layout

```
trade-enchant-cycle/
├── Cargo.toml                    # the pure core, for `cargo test`
├── compatibility.json            # game, hook API, Horizon and horizon-mixins revisions
├── core/src/lib.rs               # #[path] include of mixins/trade_enchant_cycle/core.rs
├── mixins/trade_enchant_cycle/
│   ├── horizon-mixin.toml        # the native mod: compatibility, modules, injections
│   ├── core.rs                   # config format, planner, policy, messages (no game types)
│   ├── config.rs                 # config/ file I/O + the Mods settings page
│   ├── hooks.rs                  # hook entry points, feedback, debounce, confirmation
│   ├── villager.rs               # villager reroll (injected into game/trading.rs)
│   ├── enchanting.rs             # enchanting reroll (injected into game/lan_stations.rs)
│   ├── merchant.rs               # trading-screen reroll (injected into game/lan_trading.rs)
│   ├── interact.rs  key.rs  screen_key.rs  net.rs  tick.rs
│   ├── render.rs    press.rs  release.rs  list.rs  settings.rs  click.rs
├── tests/
│   ├── villager_tests.rs
│   └── enchanting_tests.rs
├── scripts/
│   ├── build-native.ps1          # build through the launcher
│   ├── build-native.sh
│   ├── build-manual.ps1          # space-free stage (Windows path workaround) + game tests
│   └── package.py                # check dist/*.hnmod against the source, build the ZIP
├── dist/
│   ├── trade_enchant_cycle-1.1.0.hnmod
│   └── trade-enchant-cycle-1.1.0.zip
├── README.md  LICENSE  NOTICE
```

The two features are independent: each has its own module, its own config
group, its own hook entry points and its own switch. Either can be off without
touching the other.

---

## 6. Requirements

* Windows 10/11 x64, macOS on Apple Silicon, or Linux x86-64 with a Vulkan GPU.
* The Horizon launcher, with Minecraft 26.3 prepared and the Nightly
  **`nightly-2026-11-01-c9d474d`** installed.
* `cargo test`: Rust stable ≥ 1.70 (the package has no dependencies).
* The native build needs nothing else: the launcher downloads its pinned Rust
  1.99.1, MinGW-w64 on Windows, horizon-mixins and the Horizon source.

You still need to own Minecraft: Java Edition for Horizon itself.

---

## 7. Build, test and install

### 7.1 The pure tests (fast, no game)

```bash
cd trade-enchant-cycle
cargo test --offline
```

Expected: **34 passed** (18 villager + 16 enchanting, config and UI geometry).

### 7.2 The game-side tests (the real offer generator)

After staging the bundle (§7.3), from the staged source:

```bash
cargo test --locked --offline -p horizon-client --bin horizon-client tec_
```

Expected: **6 passed** — diamond sword, pickaxe, chestplate, bow and book
offers; already-enchanted items; consecutive rerolls; the item left unchanged.
`scripts/build-manual.ps1 -Tests` runs exactly this.

### 7.3 Build the native mod with the launcher (recommended)

Close the game, then:

```powershell
# Windows PowerShell (quote paths that contain spaces)
& "C:\path\to\horizon-launcher.exe" native-build `
    "C:\path\to\trade-enchant-cycle\mixins\trade_enchant_cycle" `
    --trust-native-code
```

```bash
# Linux / macOS
horizon-launcher native-build "$PWD/mixins/trade_enchant_cycle" --trust-native-code
```

The launcher downloads its toolchain and the pinned source (once), applies
`mixins/trade_enchant_cycle/horizon-mixin.toml`, compiles the game and installs a
`native-<commit>-<key>` version for the instance. The build log is
`%APPDATA%\horizon-launcher\native\build.log`.

You can also install `dist/trade_enchant_cycle-1.1.0.hnmod` from the Mods page
and press **Build** there. A native mod runs with the player's rights; the
launcher shows its red trust warning first, which is expected for any mod
outside the Horizon registry.

### 7.4 Windows: a space in `%APPDATA%`

Horizon's `crates/horizon-client/build.rs` passes `-I<CARGO_MANIFEST_DIR>` to
windres; MinGW splits it at spaces, and the build fails with

```text
cc1.exe: fatal error: Veyra\AppData\...: No such file or directory
windres: preprocessing failed.
```

That is a Horizon build-script bug, not this mod. Stage and build at a
space-free path:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-manual.ps1
```

Then install the built game into a version folder:

```powershell
$data = Join-Path $env:APPDATA "horizon-launcher"
$base = Join-Path $data "versions\nightly-2026-11-01-c9d474d"
$new  = Join-Path $data "versions\native-c9d474d-tec"
Copy-Item $base $new -Recurse
Copy-Item C:\tec-build\target\release\horizon-client.exe "$new\horizon-client.exe" -Force
@'
{"base":"nightly-2026-11-01-c9d474d","horizon":"c9d474dd9116e84c243f8992d75c11314df56f06","horizon_mixins":"0d6b103c70633d8fd9443d8d0acd3aadaacedb06","rust":"1.99.1","mods":[{"id":"trade_enchant_cycle","version":"1.1.0"}]}
'@ | Set-Content "$new\native-mods.json"
New-Item "$new\.installed" -ItemType File -Force | Out-Null
```

Select `native-c9d474d-tec` in the launcher's version picker.

### 7.5 Install the prebuilt `.hnmod`

`dist/trade_enchant_cycle-1.1.0.hnmod` is the same source in the registry
layout. Either install it from the launcher's Mods page, or, with the game
closed:

1. Copy it to `%APPDATA%\horizon\horizon-native-mods\trade_enchant_cycle.hnmod`.
2. Make sure `%APPDATA%\horizon\horizon-mods.txt` contains
   `mod.trade_enchant_cycle:true`.
3. Build with the launcher (`horizon-launcher native-build --trust-native-code`).

### 7.6 Rebuilding the archive

```bash
python scripts/package.py
```

writes `dist/trade_enchant_cycle-1.1.0.hnmod` and
`dist/trade-enchant-cycle-1.1.0.zip` from the files in this repository.

---

## 8. Horizon hooks and APIs used

| Hook / API | Level | Used for |
|---|---|---|
| `Game::native_interact_entity` (HEAD) | API 29 | A player right-clicks an entity, server-side, once per hand: the villager reroll and the claimed click |
| `Game::native_screen_key_pressed_pre` (HEAD) | screen input | The R shortcut: which screen is open, and whether the key is repeating, so one press is one reroll |
| `Game::native_key` (HEAD) | base | The same shortcut for the deprecated in-frame station path, which opens no server menu and so fires no screen key hook |
| `Game::native_screen_render_post` (HEAD) | API 78 | Drawing the Reroll bar of the open screen (enchanting or trading) |
| `Game::native_screen_mouse_button_pressed_pre` (HEAD) | API 80 | Taking a click that lands on that bar |
| `Game::native_screen_mouse_button_released_pre` (HEAD) | API 80 | Cancelling the release that ends that press, so it is not read as a click into the void (which would throw the carried stack) |
| `Game::native_tick` (HEAD) | base | Sending what a click or a key press recorded (those hooks are `&self`) |
| `Game::native_net_from_client` (HEAD) | API 40 | Hearing the client's request (`e` enchanting, `m` trading screen) |
| `Game::native_mods`, `native_settings`, `native_settings_click` (HEAD) | base | The Options → Mods row, the feature toggles and "Reload config file" |
| `Game::native_on` | base | The player's own mod on/off switch |
| `Game::native_config_dir` | API 26 | `config/trade-enchant-cycle.json` |
| `Game::native_net_send_to_server` | API 40 | The client's reroll request |
| `Game::native_action_bar`, `native_tell`, `native_chat` | API 21 / base | One-line feedback (action bar, chat fallback) |
| `Game::native_fill`, `native_text` | API 19 | The Reroll button |
| Game internals (pinned): `ensure_offers`, `offers_from_set`, `Trader`, `MerchantOffer`, `merchant::Session`, `enchant_offers`, `GuestStation`/`menu.data`, `GuestMerchant`/`lan_send_offers`, `xp_seed` | — | Reusing vanilla's own generation instead of re-implementing it |

Because the last row is not part of the stable hook surface, the bundle
**pins the Horizon commit** rather than declaring a `hooks` level.

### Authority and multiplayer

* **Single player:** the world runs on an integrated server thread, and the
  window's player is a guest of it. Both features act on the server side; the
  window only asks (enchanting) or draws.
* **Hosting on LAN:** the host's server thread runs both features for the host
  and for every guest (the guest's own sneak flag, menu and seed are used). A
  guest without the mod cannot ask for an enchanting reroll (it has no
  client half), but the host's mod does not break their game.
* **Joining someone else's server:** the protocol is unchanged. The villager
  feature does nothing (it needs the server's mob data), and the enchanting
  request is refused with a message because the server carries no mod messages.
  No client-side fake reroll is performed, so nothing desynchronises.
* **Dedicated server:** Horizon does not build native mods for dedicated
  servers yet (its own documentation says so), so this mod does not support
  them either.

### Duplicate execution and safety

* One `native_interact_entity` call per hand; a successful reroll claims the
  click, so the off-hand call and the trade screen never follow it, and a
  per-villager debounce drops a retried packet in the same ticks.
* A request is checked again on the server: the channel, the one-byte screen
  it names, the config, the player, and that player's own open menu (an
  `minecraft:enchantment` menu with an enchantable item in slot 0, or an open
  merchant whose offers are the ones rerolled). What a client sends is treated
  as that player's own input, and a request names no merchant, so it can only
  reach the requester's own trading screen.
* Every villager lookup re-checks the entity id, species, life, age and
  trading state, so a stale id (the mob removed between click and hook) is
  refused rather than written to.
* A rerolled villager keeps its `Trader` (experience, restock counters) and
  only its `offers` are replaced; the enchanting path only writes the player's
  seed and the menu's own data slots.
* A reroll from the trade screen changes the **trading session's** offer list
  (the session owns it while the screen is open, and writes it back to the
  villager when the screen closes), then sends it with `sendMerchantOffers`.
  Slot contents, the carried stack and the selection are left alone, so nothing
  the player is holding moves.

---

## 9. Tests performed (actual results)

Environment: Horizon Nightly `nightly-2026-11-01-c9d474d`, Minecraft 26.3, the
launcher's Rust 1.99.1 (`x86_64-pc-windows-gnu`), Windows 11, horizon-mixins
0.5.0 rev `0d6b103c7063`.

| Check | Result |
|---|---|
| `cargo test --offline` (project root) | **34 passed, 0 failed** |
| `horizon-mixins plan` against source `c9d474d…` | **ok** — 11 injections, 6 modules, 0 bridges/states |
| `horizon-mixins apply` to a space-free stage | **ok**, stage written |
| `cargo check --locked --offline -p horizon-client --bin horizon-client` on the staged game | **passed**, no errors, no warnings |
| `cargo test --locked --offline -p horizon-client --bin horizon-client tec_` | **6 passed, 0 failed** |
| Full build of the modded game (`cargo build -p horizon-client --bin horizon-client`) | **ok**, no warnings |
| The built game started and ran | **ok** — Vulkan renderer, world loaded, both features exercised below |

### What the tests cover

34 pure tests in two files, and 6 inside the staged game.

**Villager (pure, `tests/villager_tests.rs`)**
a villager with no unlocked trades; untouched offers replaced while used ones
are kept; a completed trade locked and never reset; offer counts at every level
1–5; every profession and level mapping to its own trade set; repeated rerolls
never duplicating an entry; enchanted books told apart by their enchantment;
`keep_used_trades: false`; profession/level/XP preserved; unknown villager
references refused; activation and confirmation; config round-trip and detail
handling.

**Enchanting (game-side, `mixins/trade_enchant_cycle/enchanting.rs`)** — run against Horizon's own
`enchant_offers` / `apply_enchantment` with a fixture registry:
diamond sword, diamond pickaxe, diamond chestplate, bow and book all get valid
costs, clues taken from their own list, and an applied enchantment equal to
that list (a book becomes an enchanted book); an already-enchanted item and an
item with no enchantability get no offers; eight consecutive seeds keep
producing valid offers and at least two differ; computing offers leaves the
stack unchanged.

**Enchanting and UI (pure, `tests/enchanting_tests.rs`)**
the documented config shape and defaults; a reroll cost refused rather than
charged; activation codes; the shortcut's key names; **where the Reroll bar
sits** — a footer on each panel's bottom edge, under the hotbar row, inside the
panel's width, clear of the window's edges at the smallest GUI — that it
follows the panel wherever the game puts it, that a click is on it only within
its own bounds (and that the panel's own hotbar row above it is not), the label
with its key; that there is no cooldown and no setting for one (a legacy
`cooldown_ticks` in the file is ignored); the Mods page toggles; every refusal's
message.

### In-game check

The built game was run with `horizon-ctl` driving a real world (creative, seed 7)
and both features were used the way a player uses them.

| Check | Result |
|---|---|
| Game starts, resources load, first frame renders | **ok** — Vulkan/AMD, chunks streaming, no error in the log |
| Config file is created on first use | **ok** — `config/trade-enchant-cycle.json`, default values, indented |
| Villager: sneak + right-click a level-2 fletcher with no offers yet | **ok** — action bar `Trades rerolled: 2 new offers.`, two valid vanilla fletcher offers (stick→emerald, 16 arrows) |
| Villager: an offer marked used (`uses: 5`) then rerolled | **ok** — action bar `Trades rerolled: 1 new offer.`, the used offer kept byte-for-byte (`uses: 5`, same prices), the other replaced with a valid one (gravel+emerald→flint), `Xp: 100` and `fletcher/level 2` unchanged |
| Villager: the trade screen did not open under the reroll click | **ok** — the click is claimed; a plain right-click still trades |
| Enchanting: the bar is a footer on the panel's bottom edge | **ok** — screenshot at GUI scale 3 (1600×900) and GUI scale 2 (800×450): under the panel, as wide as the panel less its 3 px inset, covering no offer, slot, label or inventory square |
| Enchanting: **R** with the *default* config (`activation: "interface_button"`) | **ok** — knockback 1 / unbreaking 2 / knockback 2 → sharpness 2 / knockback 1 / bane of arthropods 4, action bar `Enchanting offers rerolled.`, log `request … for [101]: EnchantingRerolled` |
| Enchanting: **R** held down for 2 s | **ok** — exactly one request in the log (the key's `repeat` flag is ignored) |
| Enchanting: **R** with the pointer over an item (tooltip up) | **ok** — offers changed; the tooltip played no part |
| Enchanting: an item tooltip that reaches the bar's strip | **ok** — the bar stays drawn and the tooltip is drawn over its end, both readable |
| Enchanting: clicking the bar (GUI scale 3, then 2) | **ok** — a request in the log and changed offers at both scales (the click is matched in the panel's own coordinates) |
| Enchanting: clicking the bar while carrying a stack | **ok** — the stack stayed on the cursor (the press *and* its release are cancelled, so nothing is thrown) |
| Enchanting: nothing is consumed by a free reroll | **ok** — the sword stayed in the table's slot unenchanted, the lapis stack unchanged (64), the player's level unchanged (30) |
| Enchanting: a real enchantment after a reroll | **ok** — the first rerolled offer (sharpness 2, cost 5) applied to the sword through vanilla's own path; the offers then cleared |
| Enchanting: an empty table | **ok** — action bar `Put an enchantable, not yet enchanted item in the table's slot.` |
| Trade screen: the bar is a footer on the panel's bottom edge | **ok** — under the list, the payment slots, the result slot and the hotbar row; 270 px wide under a 276 px panel |
| Trade screen: clicking the bar and pressing **R** | **ok** — `request … for [109]: Rerolled { added: 2 }`, action bar `Trades rerolled: 2 new offers.`, both ways |
| Trade screen: an item tooltip that reaches the bar's strip | **ok** — the hotbar item's tooltip was drawn over the bar's right end; `Reroll [R]` still readable |
| **R** in the inventory, and in the world | **ok** — no request (the screen is not one of the mod's) |
| **R** while the game is paused over the table | **ok** — no request (the pause screen is the open screen) |
| Refusals | the log records each attempt with its outcome (`Rerolled { added }`, `NotEnchantable`) |

If you build it yourself, `%APPDATA%\horizon-launcher\native\build.log` and the
game's own log are the two places a failure will show.

---

## 10. Known limitations

1. **Pinned to one Nightly.** The mod reads game internals the stable hook
   surface does not expose, so `horizon-mixin.toml` pins
   `c9d474dd…`. Another Nightly needs a re-check (and possibly small changes in
   `villager.rs` / `enchanting.rs`); the pure modules do not.
2. **Remote servers:** no reroll. The villager needs the server's mobs and the
   enchanting seed lives on the server; a client cannot change either without
   desynchronising. Single player and LAN hosts work because the mod's server
   half runs there.
3. **Dedicated servers:** not built by Horizon's native pipeline yet, so not
   supported.
4. **`--in-frame`** (Horizon's deprecated no-server-thread mode) has no server
   to ask, so the enchanting reroll reports that it is unsupported there.
5. **Used trades are kept.** A villager whose every offer was already taken
   cannot be rerolled; that is the rule, not a bug. `keep_used_trades: false`
   changes it and replaces the whole list.
6. **The offer count does not grow.** A reroll replaces the untouched offers
   and keeps the list the same length the villager had, so repeated rerolls
   cannot inflate a merchant's stock.
7. **Rerolls are free.** `free_rerolls: false` or a non-zero `reroll_cost` is
   refused with a message; the mod never charges lapis or levels.
8. **Enchanting needs the table open and an enchantable item in it.** That is
   vanilla's own condition for having offers at all.
9. **The wandering trader is opt-in** (`include_wandering_traders`), because
   its three sets are a different progression than a villager's levels.
10. **Windows path with a space:** Horizon's own build script fails at windres;
    use §7.4. Not caused by this mod.
11. **A tooltip that reaches the bar is drawn twice.** The game draws tooltips
    *before* `native_screen_render_post`, so a native mod cannot draw *under*
    one. The bar is always drawn, and when the hovered item's tooltip reaches
    its strip the tooltip is drawn again over the bar (same lines, same
    geometry, same drawing calls). The result is vanilla's order — tooltip on
    top, bar visible around it — but the second pass is a workaround for the
    hook order, not a widget: there is no API to add a real widget to a vanilla
    screen. Tooltips of the screen's own rows (a trade's entry, an enchantment
    offer) are left to the game; they sit at the top of the panel and cannot
    reach the bar.
12. **Notifications are one line** in the action bar (chat fallback). There is
    deliberately no custom GUI, texture or menu.

---

## 11. Troubleshooting

| Symptom | What to do |
|---|---|
| `These mods were made for different Horizon revisions…` | One build mixed mods pinned to different commits. Build only this mod (folder argument) or install versions made for the same Nightly |
| `… is not a Horizon commit (40 hex digits)` | Fix `horizon_revision` in `horizon-mixin.toml` |
| `E_MODULE: support module source cannot have file attributes or a shebang` | Payloads must start with `//`, not `//!` |
| `E_TARGET` / selector not found | The target moved in your Nightly. Compare `mixins/trade_enchant_cycle/horizon-mixin.toml` with `crates/horizon-client/src/game/native_hooks.rs` |
| `windres` / `cc1.exe` path split at spaces | §7.4 |
| Sneak-right-click does nothing | Check the mod is on (Options → Mods), the villager has a profession and at least one **unused** offer, and you are in single player or the LAN host |
| The trade screen opens instead of rerolling | Sneak was not held, `activation` is `sneak_right_click`, or the reroll was refused (the message says why) |
| No Reroll button | `enchanting_cycle.enabled`, `show_button`, and `activation` must allow the button; the panel must be the enchanting table's |
| "This server does not run the mod" | Joining someone else's server; see §10.2 |
| The config file is missing | It is written on the first load after install, next to `options.txt`; create the folder if you must, or press Reload |
| Want to remove it | Turn it off in Options → Mods, or remove the built version / `.hnmod`; the next launcher build without it produces a clean game |

---

## 12. Credits and licence

* Horizon and horizon-mixins (MIT) — <https://github.com/horizon-rust/horizon>,
  <https://github.com/horizon-rust/horizon-mixins>.
* Vanilla behaviour referenced through Horizon's own ports: `AbstractVillager`,
  `Villager`, `MerchantOffer`, `trade_set`/`villager_trade` data,
  `EnchantmentMenu`, `EnchantmentHelper`.
* This mod: MIT (see `LICENSE`). Not an official Minecraft product; not
  approved by or associated with Mojang or Microsoft.
