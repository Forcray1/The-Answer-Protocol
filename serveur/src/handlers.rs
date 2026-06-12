// serveur/src/handlers.rs
// Seules 2 choses ont changé par rapport à l'original :
//   1. process_command est maintenant async et reçoit le pool en paramètre
//   2. handle_connect charge les données BDD du joueur s'il existe déjà
//   3. save_player_to_db est une nouvelle fonction publique appelée depuis main

use std::net::SocketAddr;
use std::time::{Duration, Instant};
use rand::Rng;
use serde_json::json;
use tokio::sync::broadcast::Sender;

use crate::state::{ServerState, Player};
use crate::world::WorldData;
use crate::commands::GameCommand;
use crate::{GlobalEvent, log_event, DbPool};

// Point d'entrée en async

pub async fn process_command(
    addr: SocketAddr,
    command: GameCommand,
    state: &mut ServerState,
    world: &WorldData,
    tx: &Sender<GlobalEvent>,
    pool: &DbPool,
) -> (String, bool) {
    match command {
        GameCommand::Connect { username, password } => (handle_connect(addr, username, password, state, tx, pool).await, false),
        GameCommand::Look           => (handle_look(addr, state, world), false),
        GameCommand::Move(dir)      => (handle_move(addr, dir, state, world), false),
        GameCommand::Inventory      => (handle_inventory(addr, state, world), false),
        GameCommand::Info(c)        => (handle_info(addr, c, state, world), false),
        GameCommand::Equip(c)       => (handle_equip(addr, c, state, world), false),
        GameCommand::Unequip(c)     => (handle_unequip(addr, c, state, world), false),
        GameCommand::Equipment      => (handle_equipment(addr, state, world), false),
        GameCommand::Take(c)        => (handle_take(addr, c, state, world), false),
        GameCommand::Drop(c)        => (handle_drop(addr, c, state, world), false),
        GameCommand::Talk(c)        => (handle_talk(addr, c, state, world), false),
        GameCommand::Attack(c)      => (handle_attack(addr, c, state, world, tx), false),
        GameCommand::Chat { channel, message } => (handle_chat(addr, channel, message, state, tx), false),
        GameCommand::Who            => (handle_who(addr, state), false),
        GameCommand::Status         => (handle_status(addr, state), false),
        GameCommand::Quit           => ("S: OK au revoir\n".to_string(), true),
        GameCommand::Unknown        => ("S: ERR malformed_command\n".to_string(), false),
        _                           => ("S: OK commande reçue mais pas encore codée\n".to_string(), false),
    }
}

// Save appelé a la deconnexion

pub async fn save_player_to_db(pool: &DbPool, player: &Player) {
    let result = database::save_player(
        pool,
        &player.username,
        player.hp,
        player.exp,
        &player.current_room,
        &player.inventory,
        &player.equipment,
        &player.completed_quests,
    ).await;

    if let Err(e) = result {
        eprintln!("[DB] Erreur sauvegarde joueur {} : {}", player.username, e);
    } else {
        println!("[DB] Joueur {} sauvegardé.", player.username);
    }
}

// Connect adapté pour le BDD

async fn handle_connect(
    addr: SocketAddr,
    pseudo: String,
    password: String,
    state: &mut ServerState,
    tx: &Sender<GlobalEvent>,
    pool: &DbPool,
) -> String {
    if state.players.contains_key(&addr) {
        return "S: ERR you_are_already_connected\n".to_string();
    }

    if password.is_empty() {
        return "S: ERR password_required\n".to_string();
    }

    // Empêche de se connecter deux fois sur le même compte
    if state.players.values().any(|p| p.username == pseudo) {
        return "S: ERR already_logged_in\n".to_string();
    }

    // Essaie de charger un joueur existant
    match database::load_player(pool, &pseudo).await {
        Ok(Some(data)) => {
            // Compte existant → on vérifie le mot de passe avant de restaurer
            match database::verify_player(pool, &pseudo, &password).await {
                Ok(true) => {} // mot de passe correct, on continue
                Ok(false) => {
                    log_event("AUTH_FAIL", &pseudo, json!({"ip": addr.to_string()}));
                    return "S: ERR bad_credentials\n".to_string();
                }
                Err(e) => {
                    eprintln!("[DB] Erreur vérification : {}", e);
                    return "S: ERR internal_error\n".to_string();
                }
            }

            // Joueur trouvé → restaurer son état
            let player = Player {
                username: pseudo.clone(),
                hp: data.hp,
                exp: data.exp,
                current_room: data.current_room,
                inventory: data.inventory,
                equipment: data.equipment,
                completed_quests: data.completed_quests,
                last_move: None,
                last_attack: None,
                last_chat: None,
            };
            state.players.insert(addr, player);
            log_event("CONNECT", &pseudo, json!({"ip": addr.to_string(), "type": "returning"}));
            let _ = tx.send(GlobalEvent {
                sender_addr: addr,
                message: format!("S: EVT GLOBAL CHAT Serveur {} est de retour !\n", pseudo),
            });
            "S: OK connected\n".to_string()
        }
        Ok(None) => {
            // Nouveau joueur -> on crée le compte avec le mot de passe fourni
            if let Err(e) = database::register_player(pool, &pseudo, &password).await {
                eprintln!("[DB] Erreur création compte : {}", e);
                return "S: ERR internal_error\n".to_string();
            }
            state.add_player(addr, pseudo.clone());
            log_event("CONNECT", &pseudo, json!({"ip": addr.to_string(), "type": "new"}));
            let _ = tx.send(GlobalEvent {
                sender_addr: addr,
                message: format!("S: EVT GLOBAL CHAT Serveur {} vient de se connecter !\n", pseudo),
            });
            "S: OK connected\n".to_string()
        }
        Err(e) => {
            eprintln!("[DB] Erreur chargement joueur : {}", e);
            "S: ERR internal_error\n".to_string()
        }
    }
}

// A partir de la c'est ton code j'ai pas touché

fn handle_look(addr: SocketAddr, state: &ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get(&addr) {
        if let Some(loc) = world.world.locations.get(&player.current_room) {
            let mut objets_text = String::from("Aucun objet au sol.");
            if let Some(items_au_sol) = state.room_items.get(&player.current_room) {
                let noms_objets: Vec<String> = items_au_sol.iter()
                    .filter(|ri| {
                        if let Some(static_item) = world.world.items.iter().find(|i| i.id == ri.item_id) {
                            match static_item.r#type {
                                crate::world::ItemType::Standard => true,
                                crate::world::ItemType::Quest => !ri.collected_by.contains(&player.username)
                            }
                        } else { false }
                    })
                    .map(|ri| {
                        world.world.items.iter()
                            .find(|i| i.id == ri.item_id)
                            .map(|i| format!("\"{}\"", i.name))
                            .unwrap_or_else(|| ri.item_id.clone())
                    }).collect();
                if !noms_objets.is_empty() {
                    objets_text = format!("Objets au sol : {}", noms_objets.join(", "));
                }
            }
            let mut npcs_text = String::from("Personne d'autre ici.");
            if let Some(npcs_ici) = state.room_npcs.get(&player.current_room) {
                if !npcs_ici.is_empty() {
                    let noms_npcs: Vec<String> = npcs_ici.iter().map(|id| {
                        world.world.npcs.iter().find(|n| &n.id == id)
                            .map(|n| format!("\"{}\"", n.name))
                            .unwrap_or_else(|| id.clone())
                    }).collect();
                    npcs_text = format!("Présents : {}", noms_npcs.join(", "));
                }
            }
            format!("S: OK [{}] - {} | {} | {}\n", loc.name, loc.description, objets_text, npcs_text)
        } else { "S: ERR room_not_found\n".to_string() }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_move(addr: SocketAddr, dir: String, state: &mut ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get_mut(&addr) {
        if player.last_move.map_or(false, |last| last.elapsed() < Duration::from_millis(500)) {
            "S: ERR cooldown_mouvement_trop_rapide\n".to_string()
        } else if let Some(loc) = world.world.locations.get(&player.current_room) {
            if let Some(next_room) = loc.exits.get(&dir) {
                player.current_room = next_room.clone();
                player.last_move = Some(Instant::now());
                format!("S: OK room-loc.{}\n", next_room)
            } else { format!("S: ERR aucune sortie vers le {}\n", dir) }
        } else { "S: ERR room_error\n".to_string() }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_inventory(addr: SocketAddr, state: &ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get(&addr) {
        if player.inventory.is_empty() {
            "S: OK Ton inventaire est vide.\n".to_string()
        } else {
            let noms_objets: Vec<String> = player.inventory.iter().map(|(id, count)| {
                let name = world.world.items.iter()
                    .find(|i| &i.id == id)
                    .map(|i| i.name.clone())
                    .unwrap_or_else(|| id.clone());
                if *count > 1 { format!("{} (x{})", name, count) } else { name }
            }).collect();
            format!("S: OK Inventaire : [{}]\n", noms_objets.join(", "))
        }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_info(addr: SocketAddr, cible: String, state: &ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get(&addr) {
        if let Some(item) = world.world.items.iter().find(|i| i.id.to_lowercase() == cible.to_lowercase() || i.name.to_lowercase() == cible.to_lowercase()) {
            if player.inventory.contains_key(&item.id) {
                let mut stats = String::new();
                if let Some(dmg) = item.damage { stats.push_str(&format!("\n Dégâts : +{}", dmg)); }
                if let Some(def) = item.defense { stats.push_str(&format!("\n Armure : +{}", def)); }
                format!("S: OK --- {} ---\n {}{}\n", item.name, item.description, stats)
            } else { "S: ERR Tu ne possèdes pas cet objet.\n".to_string() }
        } else { "S: ERR Objet inconnu.\n".to_string() }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_equip(addr: SocketAddr, cible: String, state: &mut ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get_mut(&addr) {
        if let Some(item) = world.world.items.iter().find(|i| i.id.to_lowercase() == cible.to_lowercase() || i.name.to_lowercase() == cible.to_lowercase()) {
            if player.inventory.contains_key(&item.id) {
                if let Some(slot) = &item.slot {
                    player.equipment.insert(slot.clone(), item.id.clone());
                    format!("S: OK Tu as équipé {} dans l'emplacement [{}].\n", item.name, slot)
                } else { "S: ERR Cet objet ne peut pas être équipé.\n".to_string() }
            } else { "S: ERR Tu ne possèdes pas cet objet.\n".to_string() }
        } else { "S: ERR Objet inconnu.\n".to_string() }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_unequip(addr: SocketAddr, cible: String, state: &mut ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get_mut(&addr) {
        let mut slot_a_retirer = None;
        for (slot, item_id) in &player.equipment {
            if let Some(item) = world.world.items.iter().find(|i| &i.id == item_id) {
                if item.id.to_lowercase() == cible.to_lowercase() || item.name.to_lowercase() == cible.to_lowercase() {
                    slot_a_retirer = Some((slot.clone(), item.name.clone()));
                    break;
                }
            }
        }
        if let Some((slot, name)) = slot_a_retirer {
            player.equipment.remove(&slot);
            format!("S: OK Tu as déséquipé {}.\n", name)
        } else { "S: ERR Tu ne portes pas cet objet.\n".to_string() }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_equipment(addr: SocketAddr, state: &ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get(&addr) {
        let mut head_name   = "Aucun".to_string();
        let mut chest_name  = "Aucun".to_string();
        let mut legs_name   = "Aucun".to_string();
        let mut weapon_name = "Aucun".to_string();
        for (slot, item_id) in &player.equipment {
            let nom = world.world.items.iter().find(|i| &i.id == item_id).map(|i| i.name.clone()).unwrap_or_else(|| item_id.clone());
            match slot.as_str() {
                "head" => head_name = nom, "chest" => chest_name = nom,
                "legs" => legs_name = nom, "weapon" => weapon_name = nom, _ => {}
            }
        }
        format!("S: OK --- ÉQUIPEMENT ---\n  Tête: {}\n  Torse: {}\n  Jambes: {}\n  Arme: {}\n", head_name, chest_name, legs_name, weapon_name)
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_take(addr: SocketAddr, cible: String, state: &mut ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get_mut(&addr) {
        let salle_actuelle = player.current_room.clone();
        if let Some(item) = world.world.items.iter().find(|i| i.id.to_lowercase() == cible.to_lowercase() || i.name.to_lowercase() == cible.to_lowercase()) {
            if let Some(items_salle) = state.room_items.get_mut(&salle_actuelle) {
                if let Some(index) = items_salle.iter().position(|ri| ri.item_id == item.id) {
                    let mut deja_ramasse = false;
                    match item.r#type {
                        crate::world::ItemType::Standard => { items_salle.remove(index); }
                        crate::world::ItemType::Quest => {
                            let runtime_item = &mut items_salle[index];
                            if runtime_item.collected_by.contains(&player.username) { deja_ramasse = true; }
                            else { runtime_item.collected_by.insert(player.username.clone()); }
                        }
                    }
                    if deja_ramasse {
                        "S: ERR Tu as déjà ramassé cet objet de quête.\n".to_string()
                    } else {
                        *player.inventory.entry(item.id.clone()).or_insert(0) += 1;
                        log_event("TAKE", &player.username, json!({"item_id": item.id}));
                        let mut quest_msg = String::new();
                        for q in &world.world.quests {
                            if let crate::world::QuestObjective::FetchItem { item: target_id } = &q.objective {
                                if target_id == &item.id && !player.completed_quests.contains(&q.id) {
                                    player.completed_quests.push(q.id.clone());
                                    if let Some(xp) = q.reward_exp {
                                        player.exp += xp;
                                        quest_msg = format!(" [QUÊTE ACCOMPLIE] {} ! (+{} EXP)", q.name, xp);
                                    }
                                }
                            }
                        }
                        format!("S: OK Tu as ramassé : {}{}\n", item.name, quest_msg)
                    }
                } else { "S: ERR Cet objet n'est pas ici.\n".to_string() }
            } else { "S: ERR room_error\n".to_string() }
        } else { "S: ERR Objet inconnu.\n".to_string() }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_drop(addr: SocketAddr, cible: String, state: &mut ServerState, world: &WorldData) -> String {
    if let Some(player) = state.players.get_mut(&addr) {
        if let Some(item) = world.world.items.iter().find(|i| i.id.to_lowercase() == cible.to_lowercase() || i.name.to_lowercase() == cible.to_lowercase()) {
            if player.inventory.contains_key(&item.id) {
                if item.r#type == crate::world::ItemType::Quest {
                    "S: ERR Impossible de se débarrasser d'un objet de quête !\n".to_string()
                } else {
                    if let Some(count) = player.inventory.get_mut(&item.id) {
                        if *count > 1 { *count -= 1; } else { player.inventory.remove(&item.id); }
                    }
                    let salle_actuelle = player.current_room.clone();
                    state.room_items.entry(salle_actuelle).or_default().push(crate::state::RuntimeItem {
                        item_id: item.id.clone(),
                        source: crate::state::ItemSource::PlayerDrop,
                        collected_by: std::collections::HashSet::new(),
                    });
                    log_event("DROP", &player.username, json!({"item_id": item.id}));
                    format!("S: OK Tu as posé au sol : {}\n", item.name)
                }
            } else { format!("S: ERR Tu ne possèdes pas l'objet \"{}\".\n", item.name) }
        } else { "S: ERR Objet inconnu.\n".to_string() }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_talk(addr: SocketAddr, cible: String, state: &mut ServerState, world: &WorldData) -> String {
    let mut npc_trouve = None;
    let mut quete_validee = false;
    let mut quest_msg = String::new();
    if let Some(player) = state.players.get_mut(&addr) {
        let salle_actuelle = player.current_room.clone();
        if let Some(npcs_ici) = state.room_npcs.get(&salle_actuelle) {
            for npc_id in npcs_ici {
                if let Some(npc) = world.world.npcs.iter().find(|n| &n.id == npc_id) {
                    if npc.id.to_lowercase() == cible.to_lowercase() || npc.name.to_lowercase() == cible.to_lowercase() {
                        npc_trouve = Some(npc.clone()); break;
                    }
                }
            }
        }
        if let Some(npc) = &npc_trouve {
            for q in &world.world.quests {
                if let crate::world::QuestObjective::DeliverItem { .. } = &q.objective {
                    if q.giver_id.as_deref() == Some(&npc.id) && player.inventory.contains_key(&q.target_id) && !player.completed_quests.contains(&q.id) {
                        player.completed_quests.push(q.id.clone());
                        if let Some(count) = player.inventory.get_mut(&q.target_id) {
                            if *count > 1 { *count -= 1; } else { player.inventory.remove(&q.target_id); }
                        }
                        if let Some(ref reward_id) = q.reward_item {
                            *player.inventory.entry(reward_id.clone()).or_insert(0) += 1;
                            let item_name = world.world.items.iter().find(|i| &i.id == reward_id).map(|i| i.name.clone()).unwrap_or_else(|| reward_id.clone());
                            quest_msg = format!("\n[QUÊTE ACCOMPLIE] {} ! Tu reçois : {}.", q.name, item_name);
                        }
                        if let Some(xp) = q.reward_exp { player.exp += xp; quest_msg.push_str(&format!(" (+{} EXP)", xp)); }
                        quete_validee = true; break;
                    }
                }
            }
        }
    }
    if let Some(npc) = npc_trouve {
        if quete_validee { format!("S: OK {} prend l'objet. {}\n", npc.name, quest_msg) }
        else { format!("S: OK {} dit : \"{}\"\n", npc.name, npc.dialogue.join(" ")) }
    } else { "S: ERR Il n'y a personne de ce nom ici.\n".to_string() }
}

fn handle_attack(addr: SocketAddr, cible: String, state: &mut ServerState, world: &WorldData, tx: &Sender<GlobalEvent>) -> String {
    let mut salle_actuelle = String::new();
    let mut degats_joueur = 10;
    let mut armure_joueur = 0;
    let mut monstre_id_trouve = None;
    let mut monstre_nom = String::new();
    let mut joueur_nom = String::new();
    let mut degats_monstre = 5;
    let mut defense_monstre = 0;
    let mut exp_gagnee = 0;
    let mut drops_monstre = Vec::new();
    let mut en_cooldown = false;

    if let Some(player) = state.players.get(&addr) {
        if player.last_attack.map_or(false, |last| last.elapsed() < Duration::from_millis(1000)) { en_cooldown = true; }
    }
    if en_cooldown { return "S: ERR attack_cooldown\n".to_string(); }

    if let Some(player) = state.players.get_mut(&addr) {
        salle_actuelle = player.current_room.clone();
        joueur_nom = player.username.clone();
        player.last_attack = Some(Instant::now());
        for (slot, item_id) in &player.equipment {
            if let Some(item_def) = world.world.items.iter().find(|i| &i.id == item_id) {
                if slot == "weapon" { degats_joueur += item_def.damage.unwrap_or(0); }
                else { armure_joueur += item_def.defense.unwrap_or(0); }
            }
        }
    } else { return "S: ERR utilize_connect_first\n".to_string(); }

    if let Some(npcs_dans_salle) = state.room_npcs.get(&salle_actuelle) {
        for npc_id in npcs_dans_salle {
            if let Some(npc) = world.world.npcs.iter().find(|n| &n.id == npc_id) {
                if npc.role == "enemy" && (npc.id.to_lowercase() == cible.to_lowercase() || npc.name.to_lowercase() == cible.to_lowercase()) {
                    monstre_id_trouve = Some(npc.id.clone());
                    monstre_nom = npc.name.clone();
                    degats_monstre = npc.damage.unwrap_or(5);
                    defense_monstre = npc.defense.unwrap_or(0);
                    exp_gagnee = npc.exp_reward.unwrap_or(0);
                    drops_monstre = npc.drops.clone();
                    break;
                }
            }
        }
    }

    if let Some(m_id) = monstre_id_trouve {
        let mut monstre_mort = false;
        let mut pv_monstre_restants = 0;
        let degats_finaux = (degats_joueur - defense_monstre).max(1);
        if let Some(hp) = state.npc_hps.get_mut(&m_id) {
            *hp -= degats_finaux;
            pv_monstre_restants = *hp;
            if *hp <= 0 { monstre_mort = true; }
        }
        if monstre_mort {
            if let Some(npcs_dans_salle) = state.room_npcs.get_mut(&salle_actuelle) {
                npcs_dans_salle.retain(|id| id != &m_id);
            }
            if let Some(npc_static) = world.world.npcs.iter().find(|n| n.id == m_id) {
                let delay = npc_static.respawn_time.unwrap_or(30);
                state.dead_npcs.push(crate::state::DeadNpc {
                    npc_id: m_id.clone(), room_id: salle_actuelle.clone(),
                    respawn_at: Instant::now() + Duration::from_secs(delay),
                });
            }
            let mut noms_drops = Vec::new();
            if let Some(player) = state.players.get_mut(&addr) {
                player.exp += exp_gagnee;
                for drop in drops_monstre {
                    if rand::thread_rng().gen_range(1..=100) <= drop.chance {
                        *player.inventory.entry(drop.item_id.clone()).or_insert(0) += 1;
                        let nom = world.world.items.iter().find(|i| i.id == drop.item_id).map(|i| i.name.clone()).unwrap_or(drop.item_id.clone());
                        noms_drops.push(nom);
                    }
                }
            }
            let mut msg = format!("S: OK Tu as infligé {} dégâts. {} s'effondre ! (+{} EXP)\n", degats_finaux, monstre_nom, exp_gagnee);
            if !noms_drops.is_empty() { msg.push_str(&format!("Tu obtiens : {}\n", noms_drops.join(", "))); }
            msg
        } else {
            let mut joueur_mort = false;
            let mut pv_joueur = 0;
            let degats_monstre_finaux = (degats_monstre - armure_joueur).max(1);
            if let Some(player) = state.players.get_mut(&addr) {
                player.hp -= degats_monstre_finaux;
                pv_joueur = player.hp;
                if player.hp <= 0 {
                    joueur_mort = true;
                    player.hp = 100;
                    player.current_room = "village_square".to_string();
                }
            }
            if joueur_mort {
                let _ = tx.send(GlobalEvent { sender_addr: addr, message: format!("S: EVT GLOBAL CHAT Serveur Un joueur a été tué par {} !\n", monstre_nom) });
                log_event("DEATH", &joueur_nom, json!({"killer": monstre_nom}));
                format!("S: OK Tu infliges {} dégâts, mais {} t'achève. Tu es MORT ! Tu te réveilles au village.\n", degats_finaux, monstre_nom)
            } else {
                format!("S: OK Tu attaques {} ({} PV restants). Il riposte (-{} PV). (Tes PV: {})\n", monstre_nom, pv_monstre_restants, degats_monstre_finaux, pv_joueur)
            }
        }
    } else { "S: ERR Impossible d'attaquer ça.\n".to_string() }
}

fn handle_chat(addr: SocketAddr, channel: String, message: String, state: &mut ServerState, tx: &Sender<GlobalEvent>) -> String {
    let mut en_cooldown = false;
    let mut current_room = String::new();
    if let Some(player) = state.players.get(&addr) {
        if player.last_chat.map_or(false, |last| last.elapsed() < Duration::from_millis(2000)) { en_cooldown = true; }
        else { current_room = player.current_room.clone(); }
    }
    if en_cooldown { return "S: ERR spam_chat_interdit\n".to_string(); }
    if let Some(player) = state.players.get_mut(&addr) {
        player.last_chat = Some(Instant::now());
        let username = player.username.clone();
        match channel.as_str() {
            "GLOBAL" => {
                let _ = tx.send(GlobalEvent { sender_addr: addr, message: format!("S: EVT GLOBAL CHAT {} {}\n", username, message) });
                "S: OK\n".to_string()
            }
            "ROOM" => {
                let _ = tx.send(GlobalEvent { sender_addr: addr, message: format!("S: EVT ROOM {} CHAT {} {}\n", current_room, username, message) });
                "S: OK\n".to_string()
            }
            "GROUP" => {
                let _ = tx.send(GlobalEvent { sender_addr: addr, message: format!("S: EVT GROUP CHAT {} {}\n", username, message) });
                "S: OK\n".to_string()
            }
            _ => "S: ERR channel_inconnu\n".to_string(),
        }
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_who(addr: SocketAddr, state: &ServerState) -> String {
    let total_serveur = state.players.len();
    if let Some(current_player) = state.players.get(&addr) {
        let joueurs_piece: Vec<String> = state.players.values()
            .filter(|p| p.current_room == current_player.current_room)
            .map(|p| format!("\"{}\"", p.username))
            .collect();
        format!("S: OK {{ \"room\": [{}], \"server\": {} }}\n", joueurs_piece.join(", "), total_serveur)
    } else { "S: ERR utilize_connect_first\n".to_string() }
}

fn handle_status(addr: SocketAddr, state: &ServerState) -> String {
    if let Some(player) = state.players.get(&addr) {
        format!("S: OK PV: {}/100 | EXP: {} | Lieu: {}\n", player.hp, player.exp, player.current_room)
    } else { "S: ERR utilize_connect_first\n".to_string() }
}