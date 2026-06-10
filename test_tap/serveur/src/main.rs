// serveur/src/main.rs

use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};
use serde_json::json;
use chrono::Local;
use std::sync::atomic::{AtomicBool, Ordering};
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Duration, Instant};
use reqwest::Client;

mod commands; 
mod world; 
mod state; 

use commands::GameCommand;
use world::WorldData;
use state::ServerState;

#[derive(Clone, Debug)]
struct GlobalEvent {
    sender_addr: std::net::SocketAddr,
    message: String,
}

static LOG_MODE: AtomicBool = AtomicBool::new(false);

fn log_event(event_type: &str, player: &str, details: serde_json::Value) {
    if LOG_MODE.load(Ordering::SeqCst) {
        let log_entry = json!({
            "timestamp": Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
            "event": event_type,
            "player": player,
            "details": details
        });
        
        let log_string = log_entry.to_string();
        println!("{}", log_string);
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open("logs.json") 
        {
            let _ = writeln!(file, "{}", log_string);
        }
    }
}

async fn ask_aldous(player_name: &str, inventory: &[String], exp: i32) -> String {
    let client = Client::new();
    
    // On forge le contexte de manière dynamique
    let prompt = format!(
        "Tu es Aldous le Borgne, un vieux sage grincheux et mystérieux dans un monde dark fantasy (Ombreval). \
        Tu t'adresses directement au joueur nommé {}. Il a actuellement {} points d'expérience. \
        Son inventaire contient ces objets : {:?}. \
        Si son inventaire contient l'ID 'item_oeil_corbeau', exige qu'il te rende 'l'Œil de Corbeau' immédiatement. \
        Sinon, donne-lui un avertissement cryptique sur les dangers qui rôdent. \
        Réponds EN UNE SEULE PHRASE COURTE et très immersive. Ne sors jamais de ton rôle. Ne prononce JAMAIS les identifiants techniques comme 'item_...' à voix haute.",
        player_name, exp, inventory
    );

    let api_key = "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";
    let url = "https://api.groq.com/openai/v1/chat/completions";

    let payload = json!({
        "model": "llama-3.1-8b-instant",
        "messages": [{"role": "system", "content": prompt}],
        "max_tokens": 150,
        "temperature": 0.7
    });

    let res = client.post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await;

    match res {
        Ok(response) => {
            let raw_text = response.text().await.unwrap_or_default();
            println!("[DEBUG IA] Réponse de Groq : {}", raw_text);

            // On essaie de lire le JSON
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw_text) {
                if let Some(text) = json["choices"][0]["message"]["content"].as_str() {
                    return text.trim().to_string();
                }
            }
            "Les esprits de Llama sont confus...".to_string()
        }
        Err(e) => {
            println!("[DEBUG IA] Erreur de connexion : {}", e);
            "Je... je ne reçois plus les visions (Erreur API).".to_string()
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|arg| arg == "--logs") {
        LOG_MODE.store(true, Ordering::SeqCst);
        println!("[SERVEUR] 🛠️  Mode LOG JSON activé.");
    }

    println!("[SERVEUR] Chargement de la carte...");
    let world_data = WorldData::load_from_file("world.yaml").expect("Erreur world.yaml");
    let shared_world = Arc::new(world_data);

    let mut initial_state = ServerState::new();
    initial_state.initialize_from_world(&shared_world);

    let shared_state = Arc::new(Mutex::new(initial_state));

    let (tx, _rx) = broadcast::channel::<GlobalEvent>(32);
    let shared_tx = Arc::new(tx);

    let listener = TcpListener::bind("127.0.0.1:4243").await?;
    println!("[SERVEUR] En écoute sur le port 4243... Système de Quêtes activé.");

    loop {
        let (mut socket, addr) = listener.accept().await?;
        
        let world = Arc::clone(&shared_world);
        let state = Arc::clone(&shared_state);
        let tx = Arc::clone(&shared_tx);
        let mut rx = tx.subscribe();

        tokio::spawn(async move {
            let (reader, mut writer) = socket.split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            let _ = writer.write_all(b"S: DK hello proto=1\n").await;

            loop {
                line.clear();
                
                tokio::select! {
                    result = reader.read_line(&mut line) => {
                        match result {
                            Ok(0) => {
                                let mut guard = state.lock().await;
                                let server_state = &mut *guard;
                                if let Some(player) = server_state.remove_player(addr) {
                                    println!("[SERVEUR] {} s'est déconnecté.", player.username);
                                    let _ = tx.send(GlobalEvent { sender_addr: addr, message: format!("S: EVT GLOBAL CHAT Serveur {} a quitté le monde.\n", player.username) });
                                }
                                break;
                            }
                            Ok(_) => {
                                let commande_analysee = GameCommand::parse(&line);
                                let mut client_veut_quitter = false;
                                let mut trigger_ai = None; 
                                
                                // 🌟 1. On ouvre un bloc pour le lock mémoire. 
                                // Tout ce qui est à l'intérieur protège le serveur.
                                let mut reponse = {
                                    let mut guard = state.lock().await;
                                    let server_state = &mut *guard;

                                    match commande_analysee {
                                        GameCommand::Connect(pseudo) => {
                                            if server_state.players.contains_key(&addr) {
                                                "S: ERR you_are_already_connected\n".to_string()
                                            } else {
                                                server_state.add_player(addr, pseudo.clone());
                                                log_event("CONNECT", &pseudo, json!({"ip": addr.to_string()}));
                                                let _ = tx.send(GlobalEvent { 
                                                    sender_addr: addr, 
                                                    message: format!("S: EVT GLOBAL CHAT Serveur {} vient de se connecter !\n", pseudo) 
                                                });
                                                format!("S: OK connected\n")
                                            }
                                        }
                                        
                                        GameCommand::Look => {
                                            if let Some(player) = server_state.players.get(&addr) {
                                                if let Some(loc) = world.world.locations.get(&player.current_room) {
                                                    let mut objets_text = String::from("Aucun objet au sol.");
                                                    if let Some(items_au_sol) = server_state.room_items.get(&player.current_room) {

                                                        let noms_objets: Vec<String> = items_au_sol.iter()
                                                            .filter(|ri| {
                                                                if let Some(static_item) = world.world.items.iter().find(|i| i.id == ri.item_id) {
                                                                    match static_item.r#type {
                                                                        crate::world::ItemType::Standard => true, // Tout le monde voit les objets standards
                                                                        crate::world::ItemType::Quest => {
                                                                            !ri.collected_by.contains(&player.username)
                                                                        }
                                                                    }
                                                                } else {
                                                                    false
                                                                }
                                                            })
                                                            .map(|ri| {
                                                                world.world.items.iter()
                                                                    .find(|i| i.id == ri.item_id)
                                                                    .map(|i| format!("\"{}\"", i.name))
                                                                    .unwrap_or_else(|| ri.item_id.clone())
                                                            })
                                                            .collect();

                                                        if !noms_objets.is_empty() {
                                                            objets_text = format!("Objets au sol : {}", noms_objets.join(", "));
                                                        }
                                                    }
                                                    
                                                    let mut npcs_text = String::from("Personne d'autre ici.");
                                                    if let Some(npcs_ici) = server_state.room_npcs.get(&player.current_room) {
                                                        if !npcs_ici.is_empty() {
                                                            let noms_npcs: Vec<String> = npcs_ici.iter().map(|id| {
                                                                world.world.npcs.iter().find(|n| &n.id == id).map(|n| format!("\"{}\"", n.name)).unwrap_or_else(|| id.clone())
                                                            }).collect();
                                                            npcs_text = format!("Présents : {}", noms_npcs.join(", "));
                                                        }
                                                    }
                                                    format!("S: OK [{}] - {} | {} | {}\n", loc.name, loc.description, objets_text, npcs_text)
                                                } else { "S: ERR room_not_found\n".to_string() }
                                            } else { "S: ERR utilize_connect_first\n".to_string() }
                                        }
                                        
                                        GameCommand::Move(dir) => {
                                            if let Some(player) = server_state.players.get_mut(&addr) {
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

                                        GameCommand::Inventory => {
                                            if let Some(player) = server_state.players.get(&addr) {
                                                if player.inventory.is_empty() {
                                                    "S: OK Ton inventaire est vide.\n".to_string()
                                                } else {
                                                    let noms_inv: Vec<String> = player.inventory.iter().map(|id| {
                                                        world.world.items.iter().find(|i| &i.id == id).map(|i| i.name.clone()).unwrap_or_else(|| id.clone())
                                                    }).collect();
                                                    format!("S: OK Inventaire : [{}]\n", noms_inv.join(", "))
                                                }
                                            } else { "S: ERR utilize_connect_first\n".to_string() }
                                        }

                                        GameCommand::Take(cible) => {
                                            if let Some(player) = server_state.players.get_mut(&addr) {
                                                let salle_actuelle = player.current_room.clone();
                                                let item_existant = world.world.items.iter().find(|i| { 
                                                    i.id.to_lowercase() == cible.to_lowercase() || i.name.to_lowercase() == cible.to_lowercase() 
                                                });

                                                if let Some(item) = item_existant {
                                                    if let Some(items_salle) = server_state.room_items.get_mut(&salle_actuelle) {
                                                        if let Some(index) = items_salle.iter().position(|ri| ri.item_id == item.id) {
                                                            
                                                            let mut deja_ramasse = false;

                                                            // On applique la logique selon le type
                                                            match item.r#type {
                                                                crate::world::ItemType::Standard => {
                                                                    items_salle.remove(index);
                                                                }
                                                                crate::world::ItemType::Quest => {
                                                                    let runtime_item = &mut items_salle[index];
                                                                    if runtime_item.collected_by.contains(&player.username) {
                                                                        deja_ramasse = true;
                                                                    } else {
                                                                        runtime_item.collected_by.insert(player.username.clone());
                                                                    }
                                                                }
                                                            }

                                                            // On aiguille la réponse selon le statut de ramassage
                                                            if deja_ramasse {
                                                                "S: ERR Tu as déjà ramassé cet objet de quête.\n".to_string()
                                                            } else {
                                                                player.inventory.push(item.id.clone());
                                                                log_event("TAKE", &player.username, json!({"item_id": item.id, "item_name": item.name}));

                                                                let mut quest_msg = String::new();
                                                                for q in &world.world.quests {
                                                                    if q.r#type == "fetch_item" && q.target_id == item.id && !player.completed_quests.contains(&q.id) {
                                                                        player.completed_quests.push(q.id.clone());
                                                                        if let Some(xp) = q.reward_exp {
                                                                            player.exp += xp;
                                                                            quest_msg = format!(" 🌟 [QUÊTE ACCOMPLIE] {} ! (+{} EXP)", q.name, xp);
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

                                        GameCommand::Drop(cible) => {
                                            if let Some(player) = server_state.players.get_mut(&addr) {
                                                
                                                // 1. 🌟 On cherche d'abord l'objet dans le monde par son ID ou son NOM
                                                let item_existant = world.world.items.iter().find(|i| { 
                                                    i.id.to_lowercase() == cible.to_lowercase() || i.name.to_lowercase() == cible.to_lowercase() 
                                                });

                                                if let Some(item) = item_existant {
                                                    // 2. 🌟 Maintenant, on cherche si l'ID technique précis de cet objet est dans l'inventaire
                                                    if let Some(index_inv) = player.inventory.iter().position(|id| id == &item.id) {
                                                        
                                                        // 3. Sécurité anti-duplication pour les objets de quête
                                                        if item.r#type == crate::world::ItemType::Quest {
                                                            "S: ERR Impossible de se débarrasser d'un objet de quête !\n".to_string()
                                                        } else {
                                                            // Retrait de l'inventaire
                                                            player.inventory.remove(index_inv);
                                                            let salle_actuelle = player.current_room.clone();
                                                            
                                                            // Ajout au sol
                                                            server_state.room_items.entry(salle_actuelle).or_default().push(crate::state::RuntimeItem {
                                                                item_id: item.id.clone(),
                                                                source: crate::state::ItemSource::PlayerDrop,
                                                                collected_by: std::collections::HashSet::new(),
                                                            });
                                                            
                                                            log_event("DROP", &player.username, json!({"item_id": item.id, "item_name": item.name}));
                                                            format!("S: OK Tu as posé au sol : {}\n", item.name)
                                                        }
                                                    } else { 
                                                        format!("S: ERR Tu ne possèdes pas l'objet \"{}\" dans ton inventaire.\n", item.name) 
                                                    }
                                                } else { "S: ERR Objet inconnu.\n".to_string() }
                                            } else { "S: ERR utilize_connect_first\n".to_string() }
                                        }

                                        GameCommand::Talk(cible) => {
                                            let mut salle_actuelle;
                                            let mut npc_trouve = None;
                                            let mut quete_validee = false;
                                            let mut quest_msg = String::new();

                                            if let Some(player) = server_state.players.get_mut(&addr) {
                                                salle_actuelle = player.current_room.clone();
                                                
                                                if let Some(npcs_ici) = server_state.room_npcs.get(&salle_actuelle) {
                                                    for npc_id in npcs_ici {
                                                        if let Some(npc) = world.world.npcs.iter().find(|n| &n.id == npc_id) {
                                                            if npc.id.to_lowercase() == cible.to_lowercase() || npc.name.to_lowercase() == cible.to_lowercase() {
                                                                npc_trouve = Some(npc.clone());
                                                                break;
                                                            }
                                                        }
                                                    }
                                                }

                                                if let Some(npc) = &npc_trouve {
                                                    for q in &world.world.quests {
                                                        if q.r#type == "deliver_item" 
                                                            && q.giver_id.as_deref() == Some(&npc.id) 
                                                            && player.inventory.contains(&q.target_id) 
                                                            && !player.completed_quests.contains(&q.id) 
                                                        {
                                                            player.completed_quests.push(q.id.clone());
                                                            player.inventory.retain(|id| id != &q.target_id);
                                                            
                                                            if let Some(ref reward_id) = q.reward_item {
                                                                player.inventory.push(reward_id.clone());
                                                                let item_name = world.world.items.iter().find(|i| &i.id == reward_id).map(|i| i.name.clone()).unwrap_or_else(|| reward_id.clone());
                                                                log_event("DELIVER_ITEM", &player.username, json!({"quest_id": q.id, "item_id": reward_id, "item_name": item_name}));
                                                                quest_msg = format!("\n🌟 [QUÊTE ACCOMPLIE] {} ! Tu reçois : {}.", q.name, item_name);
                                                            }
                                                            if let Some(xp) = q.reward_exp {
                                                                player.exp += xp;
                                                                quest_msg.push_str(&format!(" (+{} EXP)", xp));
                                                            }
                                                            quete_validee = true;
                                                            break;
                                                        }
                                                    }

                                                    if !quete_validee && npc.id == "npc_vieux_sage" {
                                                        trigger_ai = Some((npc.name.clone(), player.username.clone(), player.inventory.clone(), player.exp));
                                                    }
                                                }
                                            }

                                            if let Some(npc) = npc_trouve {
                                                if quete_validee {
                                                    format!("S: OK {} prend l'objet. {}\n", npc.name, quest_msg)
                                                } else if trigger_ai.is_some() {
                                                    // Réponse vide temporaire, elle sera écrasée par l'IA juste en dessous
                                                    String::new() 
                                                } else {
                                                    let repliques = npc.dialogue.join(" ");
                                                    format!("S: OK {} dit : \"{}\"\n", npc.name, repliques)
                                                }
                                            } else { "S: ERR Il n'y a personne de ce nom ici.\n".to_string() }
                                        }

                                        GameCommand::Attack(cible) => {
                                            let mut salle_actuelle = String::new();
                                            let mut degats_joueur = 10;
                                            let mut monstre_id_trouve = None;
                                            let mut monstre_nom = String::new();
                                            let mut joueur_nom = String::new();
                                            
                                            let mut en_cooldown = false;
                                            if let Some(player) = server_state.players.get(&addr) {
                                                if player.last_attack.map_or(false, |last| last.elapsed() < Duration::from_millis(1000)) {
                                                    en_cooldown = true;
                                                }
                                            }

                                            if en_cooldown {
                                                "S: ERR attack_cooldown\n".to_string()
                                            } else {
                                                if let Some(player) = server_state.players.get_mut(&addr) {
                                                    salle_actuelle = player.current_room.clone();
                                                    joueur_nom = player.username.clone();
                                                    player.last_attack = Some(Instant::now()); 
                                                    if player.inventory.contains(&"item_epee_rouillee".to_string()) { degats_joueur += 5; }
                                                }

                                                if let Some(npcs_dans_salle) = server_state.room_npcs.get(&salle_actuelle) {
                                                    for npc_id in npcs_dans_salle {
                                                        if let Some(npc) = world.world.npcs.iter().find(|n| &n.id == npc_id) {
                                                            if npc.role == "enemy" && (npc.id.to_lowercase() == cible.to_lowercase() || npc.name.to_lowercase() == cible.to_lowercase()) {
                                                                monstre_id_trouve = Some(npc.id.clone());
                                                                monstre_nom = npc.name.clone();
                                                                break;
                                                            }
                                                        }
                                                    }
                                                }

                                                if let Some(m_id) = monstre_id_trouve {
                                                    let mut monstre_mort = false;
                                                    let mut pv_monstre_restants = 0;
                                                    let degats_monstre = 15;
                                                    
                                                    if let Some(hp) = server_state.npc_hps.get_mut(&m_id) {
                                                        *hp -= degats_joueur;
                                                        pv_monstre_restants = *hp;
                                                        log_event("COMBAT", &joueur_nom, json!({"target": monstre_nom, "damage_dealt": degats_joueur, "target_hp_left": pv_monstre_restants}));
                                                        if *hp <= 0 { monstre_mort = true; }
                                                    }

                                                    if monstre_mort {
                                                        if let Some(npcs_dans_salle) = server_state.room_npcs.get_mut(&salle_actuelle) {
                                                            npcs_dans_salle.retain(|id| id != &m_id);
                                                        }
                                                        format!("S: OK Tu as infligé {} dégâts. Le {} s'effondre sans vie !\n", degats_joueur, monstre_nom)
                                                    } else {
                                                        let mut joueur_mort = false;
                                                        let mut pv_joueur = 0;
                                                        
                                                        if let Some(player) = server_state.players.get_mut(&addr) {
                                                            player.hp -= degats_monstre;
                                                            pv_joueur = player.hp;
                                                            if player.hp <= 0 {
                                                                joueur_mort = true;
                                                                player.hp = 100;
                                                                player.current_room = "village_square".to_string();
                                                            }
                                                        }

                                                        if joueur_mort {
                                                            let _ = tx.send(GlobalEvent { sender_addr: addr, message: format!("S: EVT GLOBAL CHAT Serveur ☠️ Un joueur a été tué par {} !\n", monstre_nom) });
                                                            log_event("DEATH", &joueur_nom, json!({"killer": monstre_nom}));
                                                            format!("S: OK Tu infliges {} dégâts, mais le {} t'achève. Tu es MORT ! Tu te réveilles sur la Place d'Ombreval.\n", degats_joueur, monstre_nom)
                                                        } else {
                                                            format!("S: OK Tu attaques {} ({} PV restants). Il riposte (-{} PV). (Tes PV: {})\n", monstre_nom, pv_monstre_restants, degats_monstre, pv_joueur)
                                                        }
                                                    }
                                                } else { "S: ERR Impossible d'attaquer ça.\n".to_string() }
                                            } 
                                        }

                                        GameCommand::Chat { channel, message } if channel == "GLOBAL" => {
                                            let mut en_cooldown = false;
                                            if let Some(player) = server_state.players.get(&addr) {
                                                if player.last_chat.map_or(false, |last| last.elapsed() < Duration::from_millis(2000)) {
                                                    en_cooldown = true;
                                                }
                                            }

                                            if en_cooldown {
                                                "S: ERR spam_chat_interdit\n".to_string()
                                            } else if let Some(player) = server_state.players.get_mut(&addr) {
                                                player.last_chat = Some(Instant::now());
                                                
                                                let format_evt = format!("S: EVT GLOBAL CHAT {} {}\n", player.username, message);
                                                log_event("CHAT", &player.username, json!({"channel": channel, "message": message}));

                                                let _ = tx.send(GlobalEvent { sender_addr: addr, message: format_evt });
                                                "S: OK\n".to_string()
                                            } else { "S: ERR utilize_connect_first\n".to_string() }
                                        }

                                        GameCommand::Who => {
                                            let total_serveur = server_state.players.len();
                                            if let Some(current_player) = server_state.players.get(&addr) {
                                                let mut joueurs_piece = Vec::new();
                                                for p in server_state.players.values() {
                                                    if p.current_room == current_player.current_room { joueurs_piece.push(format!("\"{}\"", p.username)); }
                                                }
                                                format!("S: OK {{ \"room\": [{}], \"server\": {} }}\n", joueurs_piece.join(", "), total_serveur)
                                            } else { "S: ERR utilize_connect_first\n".to_string() }
                                        }

                                        GameCommand::Status => {
                                            if let Some(player) = server_state.players.get(&addr) {
                                                format!("S: OK PV: {}/100 | EXP: {} | Lieu: {}\n", player.hp, player.exp, player.current_room)
                                            } else { "S: ERR utilize_connect_first\n".to_string() }
                                        }

                                        GameCommand::Quit => {
                                            client_veut_quitter = true;
                                            "S: OK au revoir\n".to_string()
                                        },
                                        GameCommand::Unknown => "S: ERR malformed_command\n".to_string(),
                                        _ => "S: OK commande reçue mais pas encore codée\n".to_string(),
                                    }
                                };

                                if let Some((npc_name, p_name, p_inv, p_exp)) = trigger_ai {
                                    let phrase_ia = ask_aldous(&p_name, &p_inv, p_exp).await;
                                    reponse = format!("S: OK {} te scrute de son œil unique : \"{}\"\n", npc_name, phrase_ia);
                                }

                                if writer.write_all(reponse.as_bytes()).await.is_err() { break; }

                                if client_veut_quitter {
                                    let mut guard = state.lock().await;
                                    let server_state = &mut *guard;
                                    if let Some(player) = server_state.remove_player(addr) {
                                        log_event("DISCONNECT", &player.username, json!({"reason": "QUIT command"}));
                                        let _ = tx.send(GlobalEvent { sender_addr: addr, message: format!("S: EVT GLOBAL CHAT Serveur {} a quitté le monde.\n", player.username) });
                                    }
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }

                    evt_result = rx.recv() => {
                        if let Ok(evt) = evt_result {
                            if evt.sender_addr != addr {
                                if writer.write_all(evt.message.as_bytes()).await.is_err() { break; }
                            }
                        }
                    }
                }
            }
        });
    }
}