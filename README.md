# TAP — Serveur RPG Multijoueur

Projet Rust implémentant un serveur TAP avec exploration, inventaire, chat global, quêtes, PNJ et combat en temps réel via clients TCP.

## Sommaire

- [Démarrage rapide](#démarrage-rapide)
- [Connexion au serveur](#connexion-au-serveur)
- [Catalogue des commandes](#catalogue-des-commandes)
- [Carte du monde](#carte-du-monde)
- [Scénario de test complet](#scénario-de-test-complet)

## Démarrage rapide

### 1. Lancer le serveur

Depuis le dossier du serveur :

```bash
cargo run -p serveur
```

Le terminal doit confirmer :

- le chargement réussi du fichier `world.yaml` ;
- la détection des 9 lieux de la carte ;
- l’écoute réseau sur le port `4243` ;
- l’activation du système de quêtes.

Sortie attendue :

```text
[SERVEUR] En écoute sur le port 4242...
Système de Quêtes activé.
```

### 2. Lancer le client de test

Ouvrez un deuxième terminal, puis lancez le client :

```bash
cargo run -p client
```

Vous pouvez aussi utiliser un client réseau générique comme `nc` ou `telnet`.

### 3. Lancer un second client

Optionnel, mais recommandé pour tester les interactions multijoueur en direct :

```bash
cargo run -p client OU
cargo run -p client_bevy
```

## Connexion au serveur

Dès l’ouverture d’une session client, le serveur envoie un paquet de bienvenue conforme au protocole TAP :

```text
S: DK hello proto=1
```

Tant que vous n’êtes pas connecté avec un pseudonyme, les commandes de jeu renvoient une erreur de sécurité :

```text
S: ERR utilize_connect_first
```

Initialisez votre session avec :

```text
CONNECT <VotrePseudo>
```

Exemple :

```text
CONNECT Test
```

Réponse attendue :

```text
S: OK connected
```

## Catalogue des commandes

Les commandes sont interprétées par le serveur après connexion. Les noms d’objets, PNJ et ennemis sont insensibles à la casse.

### Exploration et cartographie

| Commande | Description | Réponse attendue |
| --- | --- | --- |
| `LOOK` | Inspecte la pièce actuelle. Affiche son nom, sa description, les objets au sol et les personnages présents. | `S: OK [Nom de la pièce] - Description \| Objets au sol : [...] \| Présents : [...]` |
| `MOVE <direction>` | Déplace le joueur vers une pièce adjacente. Directions possibles selon `world.yaml` : `north`, `south`, `east`, `west`. | `S: OK room-loc.<nom_nouvelle_piece>` ou `S: ERR aucune sortie vers le...` |

### Inventaire et objets

| Commande | Description | Réponse attendue |
| --- | --- | --- |
| `INVENTORY` | Affiche les objets actuellement dans le sac du joueur. | `S: OK Inventaire : [Objet1, Objet2, ...]` |
| `TAKE <nom_ou_id_objet>` | Ramasse un objet présent au sol. L’objet est retiré de la RAM pour tous les joueurs. | `S: OK Tu as ramassé : <Nom>` |
| `DROP <nom_ou_id_objet>` | Dépose un objet de l’inventaire dans la pièce actuelle. Il devient visible et ramassable par les autres joueurs. | `S: OK Tu as posé au sol : <Nom>` |

### Social et état serveur

| Commande | Description | Réponse attendue |
| --- | --- | --- |
| `CHAT GLOBAL <message>` | Diffuse un message à tous les joueurs connectés, quelle que soit leur position. | `S: OK`, puis `S: EVT GLOBAL CHAT <Pseudo> <Message>` sur les autres clients |
| `WHO` | Liste les joueurs présents dans la pièce et le nombre total de joueurs connectés. | `S: OK { "room": ["Joueur1", "Joueur2"], "server": 2 }` |
| `STATUS` | Affiche les statistiques actuelles du joueur. | `S: OK PV: <HP>/100 \| EXP: <Points> \| Lieu: <room_id>` |

### Quêtes, dialogues et combat

| Commande | Description | Réponse attendue |
| --- | --- | --- |
| `TALK <nom_ou_id_pnj>` | Engage le dialogue avec un PNJ présent dans la pièce. Si un objet de quête requis est possédé, la quête est résolue, la récompense est donnée et l’objet est consommé. | `S: OK <PNJ> dit : "..."` |
| `ATTACK <nom_ou_id_ennemi>` | Attaque un monstre hostile présent dans la pièce. Inflige 10 dégâts de base, ou 15 avec l’Épée Rouillée. Le monstre riposte avec `-15 PV`. | Échanges de coups, mort de la cible ou mort du joueur |
| `QUIT` | Déconnecte proprement le client de la socket active. | Déconnexion propre |

## Carte du monde

Agencement logique implémenté dans `world.yaml` :

```text
               [Les Catacombes] (Donjon)
                       ▲
                       │ (north)
               [Porte des Ruines] ◄────► [Marais des Agonisants]
                       ▲                         ▲
                       │ (north)                 │ (north)
[Taverne du Pendu] ◄─► [Place d'Ombreval] ◄────► [Sentier de l'Ouest]
       │               (Point de Départ)         ▲
       │ (south)       ▲                         │ (south)
       ▼               │ (south)                 │
[Sentier de l'Est] ◄──► [Cimetière Oublié] ◄─────► [Forêt des Murmures]
```

## Scénario de test complet

Ce scénario valide les systèmes principaux : réseau, RAM, chargement YAML, quêtes et combat.

### 1. Connexion

```text
CONNECT Axel
```

Résultat attendu :

- apparition sur la Place d’Ombreval ;
- `100 PV` ;
- `0 EXP`.

### 2. Analyse initiale

```text
LOOK
```

Résultat attendu :

- présence d’Aldous le Borgne ;
- aucun objet au sol.

### 3. Sécurité géographique

```text
TALK Le Colporteur Macabre
```

Résultat attendu :

```text
S: ERR Il n'y a personne de ce nom ici.
```

Le marchand se trouve à la taverne, pas dans la pièce actuelle.

### 4. Première quête — récupérer une arme

Déplacez-vous vers la taverne :

```text
MOVE east
```

Inspectez le sol :

```text
LOOK
```

Ramassez l’Épée Rouillée :

```text
TAKE Épée Rouillée
```

Résultat attendu :

```text
🌟 [QUÊTE ACCOMPLIE] S'armer pour survivre ! (+50 EXP).
```

Vérifiez ensuite :

```text
STATUS
INVENTORY
```

Résultat attendu :

- `EXP: 50` ;
- l’Épée Rouillée apparaît dans l’inventaire.

### 5. Seconde quête — livrer l’Œil de Corbeau

Allez au Marais des Agonisants :

```text
MOVE west
MOVE west
MOVE north
```

Ramassez l’objet légendaire :

```text
TAKE L'Œil de Corbeau
```

Retournez voir Aldous :

```text
MOVE south
MOVE east
```

Remettez l’objet :

```text
TALK Aldous
```

Résultat attendu :

```text
🌟 [QUÊTE ACCOMPLIE] La vision d'Aldous ! Tu reçois : Clé en Os. (+100 EXP).
```

Aldous consomme la gemme de votre inventaire et remet la récompense.

### 6. Combat et respawn

Allez traquer le monstre :

```text
MOVE south
MOVE west
```

Détectez l’ennemi :

```text
LOOK
```

Résultat attendu :

- présence du Loup des Ombres ;
- `40 PV`.

Engagez le combat :

```text
ATTACK loup des ombres
```

Avec l’Épée Rouillée, vous infligez `15` dégâts au lieu de `10`. Le loup riposte avec `-15 PV`.

Répétez l’attaque :

```text
ATTACK loup des ombres
```

Résultat attendu :

- le loup tombe à `10 PV` ;
- le joueur passe à `70 PV`.

Si vous continuez sans vous soigner et que vos PV atteignent `0`, le serveur :

- annonce le décès sur le chat global ;
- réinitialise vos PV à `100` ;
- vous téléporte sur la Place d’Ombreval.

## Timeline:

09/06/2026:
    avauclai:
        - Core Architecture: Set up an async TCP server using tokio and a custom text-command parser.
        - World Generation: Implemented dynamic loading for rooms, items, and NPCs from a world.yaml file.
10/06/2026
	avauclai
    	- Navigation & Chat: Added room-to-room movement, area scanning (look), and proximity-filtered chat.
11/06/2026:
	mlorenzo:
		- Created the classes for all instances
        - Inventory & Quests: Created item management (take, drop) and an NPC quest delivery system (talk) with EXP/item rewards.
12/06/2026:
	mlorenzo:
		- Added a database in sql persistant throughought the restart and logs of the game/players
		- Adapted the serveur/main.rs and serveur/handlers.rs to match the database
16/06/2026:
	mlorenzo:
		- Organized all the sprites, items, pnj and zones requiered for the game
		- Adapted the classes for the missing variables
27/06/2026:
	mlorenzo:
		- Added a domain crate to hold all of the common classes and identification to specify them throughout the all project
		- Adapted the classes from classes.rs in the real program
		- Changed all of the returns from the program from french to english
06/07/2026:
	mlorenzo:
		- Added a first map of the oasis in the map.rs for the bevy client
07/07/2026:
	mlorenzo:
		- Added the sprite of the skin to the DB
		- Implemented a first version of the player for bevy client
10/07/2026:
	mlorenzo:
		- Added a toggle chat for textual commands on the bevy client
		- Added the ZQSD movement
		- Added animation based on the direction with basic sprites (to be modified)
		- Added dynamic map building from the actual room of the player
11/07/2026:
	mlorenzo:
		- Added the possibility to see other player connected in your room
		- Reorganisation of the files in bevy client
19/07/2026:
	mlorenzo:
		- Improved the world.yaml to make the move between zone possible
04/08/2026:
	mlorenzo:
		- Improved world.yaml
		- Checked up on the sprites and there composition to adapt to the code
05/08/2026:
	avauclai:
		- Added a first version of the inventory UI
		- Added first version of Login Menu
		- Correction BUG affichage position joueur client bevy
06/08/2026:
	avauclai:
		- fix du bug de teleportation
		- brouillon fonctionnel de l'inventaire UI
12/08/2026:
	mlorenzo:
		- Added the move between zones through the bevy client
18/08/2026:
	mlorenzo:
		- Added collision in the maps
		- Position of player showed
	avauclai:
		- Room Transition
		- Mob function placing
		- "E" Interaction Button
19/08/2026:
	avauclai
		- Quest UI
		- Health and XP Bar UI


TODO:
	- Reorganize the Start_oasis with the new version for the perspective
	- link all the rooms V
	- Add the collisions and interactions
	- finish the world.yaml with all the informations
	- Make all of the combat system
	- Edit the map sprites marked with the flag (_not-png) as they are not in png, so only the upper layer is shown
	- Logs not working for bevy client (ah ouais ?)

PS: fait placage de MOB sur la map et implementation de l'inventory avec la key I. 
PS: Toujours le problème de perspective a corriger mais pour le reste on devrais être bon. On a pas encore la map de cave je vais l'implémenter des que je les recois
