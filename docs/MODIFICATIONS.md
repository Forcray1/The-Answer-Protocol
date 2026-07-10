## 0. Vue d'ensemble

Le point de départ était un client « terminal graphique » : une fenêtre Bevy qui
affichait du texte. On l'a transformé en client de MMORPG 2D vue de dessus :

- une **carte** en calques superposés, choisie selon la salle du joueur ;
- un **avatar** par joueur (skin animé + pseudo), qui se **déplace** ;
- la **console texte** masquée par défaut (touche T) ;
- la **visibilité des autres joueurs** sur la même carte (multijoueur) ;
- le tout **découpé en modules** faciles à retrouver.

Principe conservé du projet : **le serveur reste la source de vérité**. Le client
n'affiche que ce que le serveur lui dit (skin, salle, présence). Les positions
2D sont, elles, relayées par le serveur mais pas validées (voir §7).

---

## 1. La carte 2D (`client_bevy/src/map.rs`)

### Quoi
Affichage d'une zone comme une **pile de calques PNG** transparents superposés,
avec un plan de profondeur (axe Z) en trois bandes :

```
[ SOL ~1.. ]  <  [ ENTITÉS ~430..570 ]  <  [ AU-DESSUS 900+ ]
 sable, eau…       joueurs / mobs           canopées, toits…
```

### Pourquoi
Pour que, plus tard, un joueur puisse passer **devant** le sol mais **derrière**
une canopée d'arbre — une perspective correcte en vue de dessus.

### Comment (Rust / Bevy)
- **Plugin** : `MapPlugin` implémente `bevy::app::Plugin`. Un plugin regroupe des
  systèmes et ressources d'un même domaine et les enregistre dans l'`App`.
- **Composant marqueur** : `#[derive(Component)] struct MapLayer;` — un composant
  vide qui « étiquette » chaque sprite de calque, pour tous les retrouver et les
  détruire au changement de zone (`Query<Entity, With<MapLayer>>`).
- **Composant public** : `#[derive(Component)] pub struct YSort;` — à ajouter sur
  toute entité à trier en profondeur.
- **Tri en profondeur** : le système `y_sort_entities` tourne en `PostUpdate`
  (après la logique de jeu) et fait `transform.translation.z = BASE - y * PENTE`.
  Plus une entité est basse à l'écran (`y` petit), plus son `z` est grand, donc
  elle passe **devant**.
- **Caméra** : `Camera2dBundle` + `OrthographicProjection` avec
  `ScalingMode::AutoMin { min_width, min_height }` pour garantir que toute la
  carte tient à l'écran.

---

## 2. Carte auto-construite selon la salle (`map.rs`, `game.rs`)

### Quoi
La carte affichée = le dossier `sprites/maps/<salle>/`. Le client **découvre
tout seul** les fichiers du dossier et les empile. Convention :

```
sprites/maps/<salle>/*.png            -> calques de SOL (sous les entités)
sprites/maps/<salle>/overhead/*.png   -> calques AU-DESSUS des entités
```

Quand la salle change, l'ancienne carte est retirée et la nouvelle reconstruite.

### Pourquoi
Pour ne **rien coder par zone** : ajouter une carte = déposer des images dans un
dossier nommé comme la salle. Le nom du dossier == l'identifiant de salle envoyé
par le serveur.

### Comment (Rust / Bevy)
- **Chemin des assets résolu à la compilation** :
  `const ASSET_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../sprites");`
  - `env!` lit une variable d'environnement **au moment de la compilation** ;
    `concat!` colle des littéraux. On obtient un chemin absolu vers `sprites/`,
    donc l'appli marche quel que soit le dossier de lancement.
- **Lecture du dossier au runtime** : `std::fs::read_dir(...)` liste les fichiers.
  Bevy ne sait pas énumérer un dossier d'assets, donc on lit le disque
  directement (OK en desktop ; ne marcherait pas en WebAssembly).
- **Tri « naturel »** : `trailing_number` extrait le nombre en fin de nom pour que
  `…-2.png` passe avant `…-10.png` (un tri lexicographique les mettrait à
  l'envers). Mécanismes : itérateurs `chars().rev().take_while(...)`, `parse()`.
- **Détection de changement de salle** : le système `sync_map_to_room` lit la
  ressource `GameState` (voir §3) et une ressource `CurrentZone(Option<String>)`
  qui mémorise la carte affichée. Il ne reconstruit que si la salle a changé.
- **Création / destruction d'entités** : via `Commands` (`commands.spawn(...)`,
  `commands.entity(e).despawn()`), qui applique les changements en fin d'étape.

---

## 3. État de jeu partagé (`client_bevy/src/game.rs`)

### Quoi
Une ressource `GameState { current_room }`, alimentée en lisant les messages du
serveur.

### Pourquoi
La carte et (plus tard) d'autres systèmes ont besoin de savoir dans quelle salle
on est, sans dépendre du réseau directement.

### Comment (Rust / Bevy)
- **Ressource** : `#[derive(Resource)] pub struct GameState { pub current_room }`
  avec `impl Default`. `init_resource::<GameState>()` l'insère au démarrage.
- **Lecture d'événements** : le système `track_current_room` prend un
  `EventReader<ServerMessageEvent>` et un `ResMut<GameState>`.
- **Parsing sans allocation** : deux petites fonctions renvoient un `Option<&str>`
  (emprunt sur le message, pas de copie) :
  - `room_from_connect` : `msg.strip_prefix("S: OK connected")?` puis
    `split_whitespace().find_map(|t| t.strip_prefix("room="))`.
  - `room_from_move` : `msg.find("room-loc.")?` + calcul de la fin du mot.
  - `?` propage le `None` ; `.or_else(...)` combine les deux sources.

---

## 4. Réseau isolé dans un module (`client_bevy/src/net.rs`)

### Quoi
Tout le transport TCP : un thread tokio dialogue avec le serveur ; Bevy échange
avec lui via des canaux. Chaque ligne reçue devient un `ServerMessageEvent`.

### Pourquoi
Le moteur de jeu (Bevy) est **synchrone** par frame ; le réseau est
**asynchrone**. On isole l'async dans un thread dédié et on communique par
messages, sans bloquer le rendu.

### Comment (Rust / Bevy)
- **Pont async ↔ sync** : `crossbeam_channel::bounded` crée deux files (serveur→
  Bevy et Bevy→serveur). Les extrémités vont dans des **ressources**
  `NetworkReceiver` (privée) et `NetworkSender` (publique).
- **Thread + runtime tokio** : `std::thread::spawn` lance un thread ; dedans,
  `tokio::runtime::Runtime::new()` + `block_on(async { … })` fait tourner l'async.
- **Cadre ligne par ligne** : `BufReader::read_line` lit jusqu'au `\n` ; on
  `trim()` puis on envoie dans le canal.
- **`let … else`** : `let Ok(stream) = TcpStream::connect(...).await else { … return }`
  gère proprement l'échec de connexion (et évite un `mut` inutile).
- **Événement Bevy** : `#[derive(Event)] pub struct ServerMessageEvent(pub String)` ;
  `read_network_messages` vide le canal et fait `events.send(...)`.
- **Filtre de log** : on n'affiche pas les lignes contenant `" POS "` (trop
  fréquentes, voir §7).

---

## 5. Skin du joueur : BDD → serveur → client

### Quoi
Chaque compte a un **skin** (nom de dossier de sprite). Le serveur le communique
au client à la connexion, qui affiche l'avatar correspondant. Défaut : `default`.

### Pourquoi
Permettre des apparences différentes par joueur, stockées durablement.

### Comment (Rust / SQL / Bevy)
- **Migration SQL** : `database/migrations/0003_add_skin.sql` fait
  `ALTER TABLE players ADD COLUMN skin TEXT NOT NULL DEFAULT 'default';`.
  `sqlx::migrate!` rejoue les migrations au démarrage (voir §9 pour un piège).
- **Couche BDD** (`database/src/lib.rs`) : champ `skin: String` dans `PlayerData` ;
  `SELECT … skin` dans `load_player` ; paramètre `skin` dans `save_player`.
- **Serveur** (`serveur/src/state.rs`, `handlers.rs`) : champ `skin` dans `Player`
  (restauré depuis la BDD à la connexion), transmis dans la réponse de connexion :
  `format!("S: OK connected skin={} name={} room={}\n", …)`.
- **Client** (`player.rs`) : `spawn_local_player_on_connect` lit la réponse avec
  `strip_prefix` + `split_whitespace` + `strip_prefix("skin=")`, puis charge le
  sprite.

---

## 6. Avatar animé, pseudo, déplacement (`client_bevy/src/player.rs`)

### Quoi
Un avatar (sprite animé + pseudo affiché au-dessus) qui se déplace en **ZQSD**,
uniquement en **NSEW** (pas de diagonale), la **première touche pressée** ayant
la priorité. Animation de marche par direction.

### Pourquoi
Rendre le jeu jouable au clavier avec une perspective et une animation crédibles.

### Comment (Rust / Bevy)
- **Skin = dossier d'animations** : `sprites/skin/<skin>/<direction>/f1..f4.png`,
  avec `direction ∈ {forward, backward, left, right}`. Chargement dans un tableau
  2D via `std::array::from_fn` (initialise un `[[Handle<Image>; N]; 4]` par une
  closure indexée — pas de `Vec`, taille connue à la compilation).
- **Composants** : `LocalPlayer`, `RemotePlayer`, et `PlayerAnimation` qui porte
  les textures, la direction regardée, la frame courante et un `Timer`.
- **Pseudo** : un `Text2dBundle` **enfant** de l'avatar (`.with_children`), donc il
  suit automatiquement le parent (héritage de `Transform`). Son `z` local le place
  au-dessus des calques « overhead » pour rester lisible.
- **Clavier physique** : `KeyCode` désigne des touches **physiques** (référence
  US-QWERTY). Sur AZERTY, les touches étiquetées Z/Q/S/D sont aux positions
  physiques W/A/S/D → on utilise `KeyCode::KeyW/KeyA/KeyS/KeyD`.
- **Priorité au 1er appui** : `ButtonInput` ne donne que « pressé » / « vient
  d'être pressé ». Pour garder l'ordre d'appui, une ressource
  `HeldDirections(Vec<Dir>)` empile les touches à leur `just_pressed`, retire les
  relâchées (`retain(|d| keys.pressed(d.key()))`), et prend `.first()`.
- **Cardinal strict** : une seule `Dir` active à la fois ⇒ pas de diagonale
  possible. `enum Dir { North, South, East, West }` avec méthodes `vec()` et
  `key()`.
- **Déplacement image/seconde** : `translation += dir * VITESSE * time.delta_seconds()`
  puis `.clamp(...)` aux bords de la carte.
- **Animation partagée** : la fonction `animate(...)` (utilisée par le joueur local
  et les distants) fait avancer la frame via `Timer::tick` + `just_finished`, et
  échange la texture (`Handle<Image>`) selon direction+frame.

---

## 7. Multijoueur : voir les autres joueurs

### Quoi
Quand plusieurs joueurs sont sur la même carte, chacun voit les avatars des
autres (bon skin, pseudo) **se déplacer** en temps réel.

### Pourquoi
C'est le cœur d'un MMORPG : la présence partagée. Rien n'existait pour ça (le
client n'affichait que soi-même).

### Comment (protocole + Rust)
Ajouts au **protocole texte** (une ligne = un message) :

| Sens | Message |
|------|---------|
| client → serveur | `POS <x> <y>` (position locale) |
| serveur → salle  | `S: EVT ROOM <salle> PRESENCE ENTER <nom> <skin> <x> <y>` |
| serveur → salle  | `S: EVT ROOM <salle> PRESENCE LEAVE <nom>` |
| serveur → salle  | `S: EVT ROOM <salle> POS <nom> <x> <y>` |

- **Serveur** :
  - `commands.rs` : nouvelle variante d'`enum` `Pos { x: f32, y: f32 }` + parsing
    (`parts[1].parse::<f32>()`).
  - `state.rs` : champs `pos_x`, `pos_y` sur `Player` (runtime, non persistés).
  - `handlers.rs` : `handle_pos` mémorise la position et la **relaie** aux autres
    de la salle via un `GlobalEvent { target_room: Some(...) }` ; il renvoie une
    chaîne **vide** (pas de réponse ⇒ pas de spam).
  - **Roster** : `room_presence_roster` construit les lignes `PRESENCE ENTER` de
    tous les **autres** déjà présents, **ajoutées** à la réponse de connexion / de
    déplacement — pour qu'un arrivant voie ceux déjà là (le client lit chaque
    ligne comme un message séparé).
  - Le broadcast salle exclut l'émetteur (`if event.sender_addr != addr`) et ne
    livre qu'aux joueurs de `target_room` (`tokio::select!` dans `main.rs`).
- **Client** (`player.rs`) :
  - `send_local_position` : envoie `POS x y` au plus ~10×/s (ressource
    `PosSendTimer(Timer)`), et **seulement si la position a changé** (mémorisée
    dans un `Local<Option<Vec2>>`, un état propre au système).
  - `handle_presence_events` : parse les lignes (`split_whitespace`), fait
    apparaître un `RemotePlayer` sur `ENTER` (réutilise `spawn_player_visual`), le
    détruit sur `LEAVE` (`despawn_recursive`), met à jour sa cible sur `POS`.
  - `update_remote_players` : déplace chaque avatar distant **en douceur** vers sa
    dernière position connue (interpolation) et l'anime selon le déplacement.
  - On ignore sa propre présence via la ressource `LocalPlayerName`.

**Limite assumée :** les positions sont relayées mais **non validées** par le
serveur (confiance au client). À revoir si la triche devient un souci.

---

## 8. Monde à 12 zones + point d'apparition (`world.yaml`, serveur)

### Quoi
`world.yaml` réécrit avec **12 salles** dont le nom == le dossier de carte :
`Wild1, Wild2, City, Castle, Wild3, Start_oasis, Wild4, Wild5, Cave, Wild6,
Oasis2, Wild7`. Point d'apparition = `Start_oasis`. Comptes existants replacés
là.

### Pourquoi
Aligner les salles du serveur sur les dossiers de cartes du client, et fixer un
spawn cohérent.

### Comment
- **Renommage** des dossiers `sprites/maps/M1..M12` vers les noms de salle.
- **`serveur/src/state.rs`** : `pub const START_ROOM: &str = "Start_oasis";`
  (utilisé pour les nouveaux comptes, le respawn à la mort, et le repli).
- **Salle transmise à la connexion** : `… room=<salle>` dans la réponse, pour que
  le client construise la bonne carte **dès la connexion** (les `exits` étant
  vides pour l'instant, on ne peut pas encore la découvrir via `MOVE`).
- **BDD** : `UPDATE players SET current_room='Start_oasis'` pour les comptes
  existants (leurs anciennes salles n'existent plus).

---

## 9. Correctif : migration SQL désynchronisée

### Quoi
Au lancement, le serveur plantait avec `Migrate(VersionMismatch(2))`.

### Pourquoi
`sqlx` enregistre une **empreinte (checksum)** de chaque migration jouée et la
revérifie à chaque démarrage. Le fichier `0002_enrich_player.sql` avait été
modifié **après** avoir été appliqué à `game.db` → l'empreinte ne correspondait
plus, et `sqlx` refusait d'aller plus loin (donc de jouer la migration `0003`).

### Comment / leçon
- Le schéma de `0002` (`level`, `money`) était **déjà** présent : la modif était
  cosmétique. On a donc remis l'empreinte enregistrée à jour (correction non
  destructive), plutôt que de recréer la base et perdre les comptes.
- **Règle** : ne **jamais** modifier une migration déjà appliquée. Pour changer
  le schéma, ajouter une **nouvelle** migration (comme `0003`).

---

## 10. Console texte à bascule (`client_bevy/src/ui.rs`)

### Quoi
La zone de chat + saisie est **masquée par défaut** : **T** l'ouvre, **Échap**
la ferme. Pas de saisie tant qu'elle est fermée.

### Pourquoi
Le jeu est graphique ; le texte ne doit apparaître qu'à la demande, sans capter
les touches de déplacement.

### Comment (Rust / Bevy)
- **Visibilité hiérarchique** : la racine `ChatUiRoot` porte une `Visibility`
  qu'on bascule (`Hidden` / `Inherited`) ; Bevy **propage** la visibilité aux
  enfants (chat + saisie).
- **Ressource d'état** : `ChatConsole { open, just_opened }`.
- **T ouvre, Échap ferme** (pas T pour fermer, sinon on ne pourrait jamais taper
  la lettre « t » dans les commandes). Le `just_opened` sert à **avaler** le « t »
  déclencheur : `std::mem::take(&mut console.just_opened)` le lit et le remet à
  `false` d'un coup.
- **Ordre des systèmes** : `handle_inputs.after(toggle_chat)` garantit qu'on lit
  l'état d'ouverture à jour dans la même frame.
- **Saisie** : `EventReader<ReceivedCharacter>` pour les caractères ; `Entrée`
  envoie la commande via `NetworkSender` ; on garde les `MAX_CHAT_LINES` dernières
  lignes.

---

## 11. Découpage en modules (lisibilité)

### Quoi
Le gros `main.rs` a été éclaté en modules, un par domaine. `main.rs` ne fait plus
que le **câblage**.

### Pourquoi
Retrouver et modifier chaque partie facilement, avec des frontières nettes et une
surface `pub` minimale.

### Comment (Rust / Bevy)
| Fichier | Rôle | Type public exporté |
|---------|------|---------------------|
| `main.rs` | câblage (`add_plugins`) | — |
| `net.rs` | réseau TCP ↔ Bevy | `NetworkPlugin`, `ServerMessageEvent`, `NetworkSender` |
| `game.rs` | état de jeu (salle) | `GamePlugin`, `GameState` |
| `ui.rs` | console texte | `ConsolePlugin`, `ChatConsole` |
| `map.rs` | carte de la zone | `MapPlugin`, `YSort` |
| `player.rs` | avatars (local + distants) | `PlayerPlugin`, `LocalPlayer` |

- **`mod` + `Plugin`** : chaque fichier est un `mod` déclaré dans `main.rs` et
  expose un `Plugin`. `App::add_plugins((A, B, C, …))` les enregistre (un tuple de
  plugins).
- **Visibilité** : seuls les types partagés entre modules sont `pub` ; le reste
  (canaux internes, composants d'UI, etc.) reste privé à son module.
- **Dépendances** : `ui`/`player` utilisent `net::{NetworkSender,
  ServerMessageEvent}` ; `map` utilise `game::GameState` ; `player` utilise
  `ui::ChatConsole` et `map::YSort`.

---

## Annexe — récapitulatif du protocole (texte, une ligne = un message)

**Connexion / salle :**
- `CONNECT <pseudo> <mdp>` → `S: OK connected skin=<skin> name=<pseudo> room=<salle>`
- `MOVE <dir>` → `S: OK room-loc.<salle>` (+ roster des présents)

**Présence & position (diffusées aux joueurs de la salle) :**
- `S: EVT ROOM <salle> PRESENCE ENTER <nom> <skin> <x> <y>`
- `S: EVT ROOM <salle> PRESENCE LEAVE <nom>`
- `POS <x> <y>` (client → serveur) → `S: EVT ROOM <salle> POS <nom> <x> <y>`

---

## Annexe — mécanismes Rust / Bevy employés (index rapide)

- **Bevy ECS** : `Plugin`, `Component`, `Resource`, `System`, `Query`,
  `Commands`, `EventReader`/`EventWriter`, ordonnancements `Startup`/`Update`/
  `PostUpdate`, `.after(...)`, `Res`/`ResMut`, `Local<T>`, `Timer`, `Transform`,
  `SpriteBundle`, `Text2dBundle`, `Visibility` (héritée), `AssetServer`,
  `ScalingMode`.
- **Rust** : `enum` + `match`, newtypes, `Option`/`Result` + `?`, `strip_prefix`,
  `find_map`, itérateurs (`take_while`, `retain`, `find_map`), `std::array::from_fn`,
  `let … else`, `std::mem::take`, closures, `concat!`/`env!` (compile-time),
  `std::fs::read_dir`, maths `Vec2` (glam).
- **Async / réseau** : thread + runtime `tokio`, `crossbeam_channel`, cadre ligne
  par ligne (`read_line`), `tokio::select!` (côté serveur).
- **Persistance** : migrations `sqlx` (+ empreintes), `ALTER TABLE`, colonnes JSON,
  Argon2 (mots de passe, existant).
