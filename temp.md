Listes des sprites a avoir et a mettre dans sprites/skin au format png
	
	Elyesa
	Remy

Faire les items au sol, dans chaque room metter au moins un item au sol ramassable par un seul joueur, qui disparait après avoir été récuperer. Laisser la possibilité de drop un de ses items au sol, sur le client bevy, il sera poser a l'emplacement du joueur et aura son sprite au sol. L'interaction pour le ramasser sera la même que pour les PNJ, ou la bulle d'interaction. Sur le client terminal, il sera juste rajouter a la room, et sera ramasser par l'action TAKE.

Faire sur les panneaux dans les oasis, des quêtes qui demandent de tuer entre 2 et 10mob, hors boss (le basilic dans cave4 et le dragon dans temple4), et récompensera en gold, entre 10 et 20 par mob demander de tuer. Sur le panneau il faut 6quêtes, tant qu'une quête n'est pas prise elle reste sur le panneau, et tant que la quête n'est pas finie, elle reste affichée sur le panneau avec le nom du joueur l'ayant prise, et ne peut pas etre acceptée. Une fois une quête finie, une nouvelle viens prendre sa place sur le panneau. Les deux panneaux ont les même quêtes, une quête prise sur le panneau de l'oasis1 ne pourra donc pas être prise sur le panneau d'Oasis2. Une option d'abandon de cette quête doit etre présente, ce qui remettra la quête accessible aux autres joueurs, en repartant de zero, et sera retirée des quêtes du joueur l'ayant abandonner. Il faut séparer le système de ces quêtes avec un fichier de generation de quête, qui prendra un mob aléatoire, une récompense en gold adaptée, et le lieu ou trouver le mob en question. Les mobs et leur spawn ne sont pas encore fait, donc il faut le module de quête marchant pour les mobs déjà présent, et sur lequel on peut ajouter les mobs que l'on ajoutera plus tard. Tout doit être écrit en Anglais, le code, les commentaires, les logs, les informations, etc...

faire dans ce fichier un recap rapide des methodes utilisée, de la structure misent, et une rapide explication de comment les systèmes marchent

---

## Implementation Recap

### Ground Items System

**Structure:**
- `RuntimeItem` (`state.rs`) gains `pos_x`/`pos_y`. World items are spread in a circle around room centre on startup; dropped items use the player's current position.
- `room_item_roster()` in `handlers.rs` builds `S: EVT ROOM ITEM_SPAWN <id> <name> <sprite> <x> <y>` for every item currently in a room.
- Sent on room enter (`handle_move`, `handle_connect`) and after every `DROP`.
- `handle_take` broadcasts `S: EVT ROOM ITEM_DESPAWN <id>` to the room when a standard item is picked up.
- `handle_drop` broadcasts `S: EVT ROOM ITEM_SPAWN` to the whole room so every client sees it appear.

**Client (`mob.rs`):**
- `GroundItem` component marks ground-item sprites. `GroundItemTarget` resource holds the id of the closest one (within 140 units).
- `handle_ground_item_events` spawns/despawns `SpriteBundle` entities on ITEM_SPAWN/ITEM_DESPAWN events; cleared on room change.
- `update_ground_item_target_impl` runs each frame, finds nearest item.
- `handle_interaction_input` checks `GroundItemTarget` first (E key → `TAKE <id>`), falls back to `INTERACT <npc_id>`.

**World:**
- 10 simple items added to `world.yaml` items list: gold_coin, iron_scrap, old_flask, herb_bundle, dusty_scroll, cracked_gem, leather_pouch, worn_key, cave_crystal, temple_relic.
- Every room now has at least one item in its `items:` list.

---

### Board Quest System

**Architecture (`board_quests.rs`):**
- `MOB_POOL` static table of `(mob_id, display_name, location_hint)`. Add new mobs here to extend the quest pool.
- `BoardQuestSlot` holds one generated quest: mob target, kill count (2-10), gold reward (kill_count x 10-20/kill), owner and progress.
- `BoardQuestPool` holds exactly 6 slots generated at startup. Both boards share the single `ServerState::board_quests` instance — taking from one locks the quest everywhere.

**Server (`commands.rs`, `handlers.rs`):**
- `BOARD_QUESTS` → `S: EVT BOARD_QUESTS <wire>` — all 6 slots, pipe-separated.
- `BOARD_TAKE <1-6>` → assigns an available slot (one per player). Broadcasts updated list.
- `BOARD_ABANDON` → releases the slot, resets kills. Broadcasts updated list.
- INTERACT with `Panneau` NPC (role `"board"`) automatically triggers the listing.
- `process_board_quest_kill()` called on mob death — increments kills_done; on completion gives gold, regenerates the slot.

**World (`world.yaml`):**
- `Panneau` NPC (role `"board"`) added to `Start_oasis` and `Oasis2`.

**Client (`board_quest_ui.rs`):**
- `BoardQuestState` resource holds parsed slots, open flag, cursor, local player name, current held slot.
- `render_board_quest_window` rebuilds on state change: white = available, green = own quest with progress, grey = taken by other.
- `handle_board_quest_input` — Arrow keys navigate, Enter to take/abandon, Esc to close.
