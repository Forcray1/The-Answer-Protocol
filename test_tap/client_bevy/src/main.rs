use bevy::prelude::*;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::thread;
use tokio::net::TcpStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

// --- LES ÉVÉNEMENTS ---
// Ces structures serviront à transmettre les messages du serveur vers le moteur de jeu Bevy
#[derive(Event)]
struct ServerMessageEvent(String);

// --- LA RESSOURCE RÉSEAU ---
// La "boîte aux lettres" que Bevy consultera à chaque frame
#[derive(Resource)]
struct NetworkReceiver(Receiver<String>);

#[derive(Resource)]
struct NetworkSender(Sender<String>);

// --- L'ÉTAT DU JEU ---
#[derive(Resource)]
struct GameState {
    current_room: String,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            current_room: "unknown".to_string(),
        }
    }
}

// --- LES COMPOSANTS UI ---
#[derive(Component)]
struct ChatText; // Un marqueur pour retrouver notre texte de chat

#[derive(Component)]
struct InputText; // Un marqueur pour le champ de saisie

// 🧠 SYSTÈME : Le cerveau du client (Parse les messages sans utiliser de split)
fn parse_server_messages(
    mut events: EventReader<ServerMessageEvent>,
    mut game_state: ResMut<GameState>,
) {
    for ev in events.read() {
        let msg = &ev.0;

        // 1. Détection d'un changement de salle (ex: "S: OK room-loc.taverne_sombre")
        let motif_salle = "room-loc.";
        if let Some(index_depart) = msg.find(motif_salle) {
            // On calcule où commence exactement le nom de la salle
            let debut_mot = index_depart + motif_salle.len();
            let mut fin_mot = debut_mot;

            // On avance notre curseur 'fin_mot' jusqu'à trouver un espace ou un saut de ligne
            for c in msg[debut_mot..].chars() {
                if c.is_whitespace() || c == '\n' || c == '\r' {
                    break;
                }
                fin_mot += c.len_utf8();
            }

            // On extrait proprement la sous-chaîne
            let room_id = &msg[debut_mot..fin_mot];
            
            // Si c'est une nouvelle salle, on met à jour la mémoire globale
            if game_state.current_room != room_id {
                game_state.current_room = room_id.to_string();
                println!("[PARSEUR] 🗺️ Déplacement détecté ! Nouvelle zone mémorisée : {}", game_state.current_room);
            }
        }
        
        // (On pourra ajouter d'autres détections ici plus tard, comme les PV ou l'inventaire)
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_event::<ServerMessageEvent>()
        .init_resource::<GameState>() // 🌟 On initialise la mémoire du jeu
        .add_systems(Startup, (setup_camera, setup_network, setup_ui))
        .add_systems(Update, (
            read_network_messages, 
            handle_inputs, 
            update_chat_ui, 
            parse_server_messages // 🌟 On active notre parseur à 60 FPS
        ))
        .run();
}

// 🎥 SYSTÈME : Initialise la caméra 2D
fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
    println!("[CLIENT] Caméra 2D initialisée, prête pour les assets pré-calculés.");
}

// 🎨 SYSTÈME : Initialise l'Interface Utilisateur (HUD)
fn setup_ui(mut commands: Commands) {
    // Conteneur principal (Colonne, aligné en bas à gauche)
    commands.spawn(NodeBundle {
        style: Style {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::FlexEnd, // Pousse le contenu vers le bas
            align_items: AlignItems::FlexStart,       // Aligne à gauche
            ..default()
        },
        ..default()
    })
    .with_children(|parent| {
        // 1. La fenêtre de chat
        parent.spawn(NodeBundle {
            style: Style {
                width: Val::Px(600.0),
                height: Val::Px(250.0),
                margin: UiRect { left: Val::Px(15.0), bottom: Val::Px(5.0), top: Val::Px(0.0), right: Val::Px(0.0) },
                padding: UiRect::all(Val::Px(15.0)),
                flex_direction: FlexDirection::ColumnReverse,
                overflow: Overflow::clip(),
                ..default()
            },
            background_color: Color::rgba(0.0, 0.0, 0.0, 0.8).into(),
            ..default()
        })
        .with_children(|parent| {
            parent.spawn((
                TextBundle::from_section(
                    "Connexion en cours...\n",
                    TextStyle { font_size: 20.0, color: Color::WHITE, ..default() },
                ),
                ChatText,
            ));
        });

        // 2. Le champ de saisie
        parent.spawn(NodeBundle {
            style: Style {
                width: Val::Px(600.0),
                height: Val::Px(40.0),
                margin: UiRect { left: Val::Px(15.0), bottom: Val::Px(15.0), top: Val::Px(0.0), right: Val::Px(0.0) },
                padding: UiRect { left: Val::Px(15.0), right: Val::Px(15.0), top: Val::Px(8.0), bottom: Val::Px(8.0) },
                ..default()
            },
            background_color: Color::rgba(0.1, 0.1, 0.1, 0.9).into(),
            ..default()
        })
        .with_children(|parent| {
            parent.spawn((
                TextBundle::from_section(
                    "> ", // On met un petit chevron jaune pour le style
                    TextStyle { font_size: 20.0, color: Color::YELLOW, ..default() },
                ),
                InputText, // 🌟 Notre nouveau marqueur est ici !
            ));
        });
    });
}

// 🌐 SYSTÈME : Initialise la connexion au serveur en arrière-plan
fn setup_network(mut commands: Commands) {
    let (tx_to_bevy, rx_in_bevy) = bounded::<String>(100);
    let (tx_to_server, rx_in_tokio) = bounded::<String>(100);

    commands.insert_resource(NetworkReceiver(rx_in_bevy));
    commands.insert_resource(NetworkSender(tx_to_server));

    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Ok(mut stream) = TcpStream::connect("127.0.0.1:4242").await {
                println!("[RÉSEAU] Connecte au serveur d'Ombreval !");
                let _ = tx_to_bevy.send("Systeme Reseau Initialise.".to_string());

                // 🌟 NOUVEAU : On sépare le flux réseau en deux (lecture / écriture)
                let (reader, mut writer) = stream.into_split();
                let mut buf_reader = BufReader::new(reader);

                // 🎧 TÂCHE 1 : Écouter le serveur en continu
                let tx_clone = tx_to_bevy.clone();
                tokio::spawn(async move {
                    let mut line = String::new();
                    loop {
                        line.clear();
                        match buf_reader.read_line(&mut line).await {
                            Ok(0) => {
                                let _ = tx_clone.send("S: ERR Connexion perdue avec le serveur.".to_string());
                                break;
                            }
                            Ok(_) => {
                                // On transmet la ligne nettoyée à Bevy
                                let _ = tx_clone.send(line.trim().to_string());
                            }
                            Err(_) => break,
                        }
                    }
                });

                // 🗣️ TÂCHE 2 : Envoyer les messages de Bevy vers le serveur
                loop {
                    // On regarde si Bevy a glissé un message dans la boîte aux lettres
                    while let Ok(message) = rx_in_tokio.try_recv() {
                        if writer.write_all(message.as_bytes()).await.is_err() {
                            return; // En cas d'erreur fatale, on quitte la boucle
                        }
                    }
                    // Petite pause de 10ms pour ne pas surcharger le processeur
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            } else {
                println!("[RÉSEAU] Impossible de joindre le serveur.");
            }
        });
    });
}

// 📬 SYSTÈME : Bevy vérifie sa boîte aux lettres à chaque frame (60 fois par seconde)
fn read_network_messages(
    receiver: Res<NetworkReceiver>,
    mut events: EventWriter<ServerMessageEvent>,
) {
    // try_recv() ne bloque pas le jeu. S'il n'y a rien, on passe à la suite instantanément.
    while let Ok(message) = receiver.0.try_recv() {
        println!("[BEVY A REÇU] : {}", message);
        events.send(ServerMessageEvent(message));
    }
}

// ⌨️ SYSTÈME : Capturer les touches et la saisie dynamique
fn handle_inputs(
    mut char_evr: EventReader<ReceivedCharacter>,
    keys: Res<ButtonInput<KeyCode>>,
    sender: Res<NetworkSender>,
    mut query: Query<&mut Text, With<InputText>>, // On cible notre champ de saisie
) {
    if let Ok(mut text) = query.get_single_mut() {
        // 1. Saisie classique (les lettres tapées)
        for ev in char_evr.read() {
            let s = ev.char.to_string();
            // On filtre pour éviter d'imprimer les caractères de contrôle (Entrée, Retour Arrière caché, etc.)
            if !s.contains('\u{8}') && !s.contains('\r') && !s.contains('\n') {
                text.sections[0].value.push_str(&s);
            }
        }

        // 2. Retour Arrière (Backspace)
        if keys.just_pressed(KeyCode::Backspace) {
            let mut current_text = text.sections[0].value.clone();
            if current_text.chars().count() > 2 { // On protège le "> " initial
                current_text.pop();
                text.sections[0].value = current_text;
            }
        }

        // 3. Envoyer la commande (Entrée)
        if keys.just_pressed(KeyCode::Enter) {
            let current_text = text.sections[0].value.clone();
            let command = current_text[2..].trim(); // On retire le "> " pour récupérer la commande pure
            
            if !command.is_empty() {
                // On envoie au serveur avec un saut de ligne
                let _ = sender.0.send(format!("{}\n", command)); 
                // On vide le champ de saisie
                text.sections[0].value = "> ".to_string(); 
            }
        }
    }
}

// 📝 SYSTÈME : Met à jour la boîte de dialogue quand le serveur parle
fn update_chat_ui(
    mut events: EventReader<ServerMessageEvent>,
    mut query: Query<&mut Text, With<ChatText>>,
) {
    for ev in events.read() {
        for mut text in query.iter_mut() {
            text.sections[0].value.push_str(&format!("{}\n", ev.0));
            
            let lines: Vec<&str> = text.sections[0].value.lines().collect();
            // 🌟 CORRECTION : On abaisse la limite à 8 lignes logiques
            if lines.len() > 8 {
                text.sections[0].value = lines[lines.len() - 8..].join("\n") + "\n";
            }
        }
    }
}