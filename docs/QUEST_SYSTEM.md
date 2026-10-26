# Quest System — Design & Mechanics

> **The Answer Protocol** — Technical Reference  
> Last updated: October 2026

---

## 1. Vue d'ensemble

Le système de quêtes est découpé en trois couches :

| Couche | Fichier(s) | Rôle |
|--------|-----------|------|
| **Domain (partagé)** | `domain/src/quest.rs` | Types communs : stades, objectifs, progression |
| **Serveur** | `serveur/src/world.rs`, `handlers.rs`, `state.rs` | Logique, déclenchement, persistance |
| **Client Bevy** | `client_bevy/src/quest.rs`, `interaction.rs` | Tracker local, UI dialogue + choix |

Le client console (`client/src/main.rs`) est purement TCP — il reçoit les mêmes
événements texte et peut les afficher sans code supplémentaire.

---

## 2. Stades de quête (`QuestStage`)

```
NotStarted  →  Stage(1)  →  Stage(2)  →  …  →  Stage(N)  →  Completed
```

| Valeur | Fil texte (`to_wire`) | Signification |
|--------|-----------------------|---------------|
| `NotStarted` | `"notstarted"` | Quête non encore déclenchée |
| `Stage(n)` | `"1"`, `"2"`, … | En cours, étape n |
| `Completed` | `"completed"` | Terminée, récompense donnée |

Le serveur envoie `S: EVT QUEST_UPDATE <quest_id> <stage_wire> <counter>` à
chaque changement de stade.

---

## 3. Types d'objectifs (`StageObjective`)

Chaque **stade** d'une quête peut porter exactement **un** objectif :

### 3.1 `TalkToNpc { npc: NpcId }`
Le joueur doit parler au PNJ indiqué (commande `TALK <name>` ou `INTERACT <name>`).  
Le serveur avance le stade automatiquement lors de l'interaction.

### 3.2 `FetchItem { item: ItemId }`
Le joueur doit rapporter un objet. La vérification se fait quand le joueur parle au
PNJ donneur de quête (`giver_id`) avec l'objet dans son inventaire.

### 3.3 `KillMob { npc: NpcId, count: u32 }`
Tuer `count` fois le mob désigné. Le compteur est incrémenté à chaque mise à mort
dans `handle_attack`. Quand `counter >= count`, le stade avance automatiquement.

### 3.4 `AnswerDialogue { npc: NpcId, expected_answer: String }`
Le serveur envoie un dialogue à choix multiples (`S: EVT DIALOGUE_CHOICE`).  
Le client affiche les boutons de réponse ; le joueur clique → le client envoie
`ANSWER <npc_id> <answer>`.  
Le serveur compare à `expected_answer` (insensible à la casse) et avance si correct.

---

## 4. Structure YAML d'une quête multi-stades

```yaml
quests:
  - id: "quest_bandit_problem"
    name: "The Bandit Problem"
    description: "Clear the bandits threatening the road."
    giver_id: "Gragas"
    # Objectif legacy (compatibilité avec l'ancien système)
    objective:
      type: talk_to_npc
      npc: Gragas
    # Stades multi-étapes (nouveau système)
    stages:
      - stage: 1
        objective:
          type: kill_mob
          npc: SandMonster
          count: 3
      - stage: 2
        objective:
          type: talk_to_npc
          npc: Gragas
    reward_exp: 150
    reward_item: "iron_sword"
```

> **Règle** : si `stages` est présent et non vide, le système multi-stades prend le
> dessus. Le champ `objective` de premier niveau reste obligatoire pour la
> compatibilité des anciennes quêtes (format legacy sans `stages`).

---

## 5. Dialogues PNJ conditionnels

Un PNJ peut avoir des blocs de dialogue différents selon la quête et le stade.

### 5.1 Champ `quest_dialogues` dans le YAML

```yaml
npcs:
  - id: "Gragas"
    name: "Sir Gragas"
    role: "Quest_giver"
    # ...
    dialogue:
      - "Hey, I have something for you to do!"   # dialogue par défaut
    quest_dialogues:
      # Stade 0 → quête pas encore démarrée (premier contact)
      - quest_id: "quest_bandit_problem"
        stage: 0
        lines:
          - "There are bandits on the road, can you help us?"
          - "Three of them have been spotted near the oasis."

      # Stade 1 → en train de tuer des monstres
      - quest_id: "quest_bandit_problem"
        stage: 1
        lines:
          - "Have you dealt with the bandits yet?"

      # Stade 2 → retour après kills, parler au PNJ
      - quest_id: "quest_bandit_problem"
        stage: 2
        lines:
          - "You did it! Excellent work. Here is your reward."

      # Stade 255 → quête terminée
      - quest_id: "quest_bandit_problem"
        stage: 255
        lines:
          - "Thank you again, adventurer. The road is safe."
```

### 5.2 Priorité de sélection

1. Cherche un bloc `quest_dialogues` correspondant à la quête + stade courant.
2. Si plusieurs quêtes sont liées au PNJ, prend la première trouvée.
3. Si aucun bloc ne correspond, affiche `dialogue` (liste par défaut).

### 5.3 Dialogue à choix multiples (`AnswerDialogue`)

```yaml
quest_dialogues:
  - quest_id: "quest_riddle"
    stage: 1
    lines:
      - "I am always hungry and must be fed."
      - "The finger I touch will soon turn red."
      - "What am I?"
    choices:
      - text: "Fire"
        answer: "fire"
      - text: "A sword"
        answer: "sword"
      - text: "The sun"
        answer: "sun"
```

Le serveur envoie `S: EVT DIALOGUE_CHOICE <npc_key> <pages>||CHOICES||<text>:<ans>|…`.  
Le client Bevy affiche les boutons après la dernière page.  
Le joueur clique → client envoie `ANSWER <npc_id> <answer>` → serveur vérifie.

---

## 6. Événements réseau

| Direction | Format | Description |
|-----------|--------|-------------|
| Serveur → Client | `S: EVT QUEST_UPDATE <id> <stage> <counter>` | Changement de stade |
| Serveur → Client | `S: EVT QUEST_DATA <entries>\|…` | Liste complète des quêtes actives |
| Serveur → Client | `S: EVT DIALOGUE <npc_key> <page1>\|<page2>\|…` | Dialogue simple |
| Serveur → Client | `S: EVT DIALOGUE_CHOICE <npc_key> <pages>\|\|CHOICES\|\|<text>:<ans>\|…` | Dialogue à choix |
| Client → Serveur | `TALK <npc_name>` | Parler à un PNJ |
| Client → Serveur | `INTERACT <npc_name>` | Interagir (parle si PNJ, combat si ennemi) |
| Client → Serveur | `ANSWER <npc_id> <answer>` | Répondre à un dialogue à choix |
| Client → Serveur | `QUESTS` | Demander la liste des quêtes actives |

`S: EVT QUEST_DATA` utilise le format :
```
<id>:<name>:<description>:<objective_text>:<stage_wire>|<id2>:…
```

---

## 7. Flux complet — exemple KillMob

```
Joueur parle à Gragas
  → Serveur: Gragas est giver_id de quest_bandit_problem, quête non commencée
  → start_quest() → quest_progress = [{ quest_id: "quest_bandit_problem", stage: Stage(1), counter: 0 }]
  → S: EVT QUEST_UPDATE quest_bandit_problem 1 0
  → Dialogue conditionnel pour stage=0 envoyé

Joueur tue 3 SandMonsters
  → handle_attack → mob mort → StageObjective::KillMob { count: 3 }
  → counter: 1, 2, 3
  → counter >= count → advance_quest_stage → Stage(2)
  → S: EVT QUEST_UPDATE quest_bandit_problem 2 0

Joueur parle à nouveau à Gragas
  → Stage(2) objectif = TalkToNpc { npc: Gragas }
  → C'est le dernier stade → reward_quest() → Completed
  → S: EVT QUEST_UPDATE quest_bandit_problem completed 0
```

---

## 8. Flux complet — exemple AnswerDialogue

```
Joueur parle au PNJ Sphinx
  → Quête quest_riddle au stage 1
  → quest_dialogue stage=1 contient des choices
  → Serveur: S: EVT DIALOGUE_CHOICE Sphinx ...||CHOICES||Fire:fire|...

Client affiche les boutons après la dernière page
  → Joueur clique "Fire"
  → Client: ANSWER Sphinx fire

Serveur: expected_answer = "fire", match!
  → advance_quest_stage ou complete selon si c'est le dernier stade
  → S: EVT QUEST_UPDATE quest_riddle 2 0  (ou completed)
```

---

## 9. Persistance

La progression est stockée dans la table `players` de la BDD via
`database::save_player` / `database::load_player`.  
La colonne `active_quests` (tableau JSON) contient désormais les entrées
`{ quest_id, stage, counter }`.

> **Migration** : les anciens enregistrements contenant uniquement des
> `QuestId` (format pré-système multi-stades) seront ignorés au chargement
> (désérialisation laxiste). Il faudra une migration SQL si la base existante
> stockait ces données.

---

## 10. Fichiers modifiés / créés

| Fichier | Action | Résumé |
|---------|--------|--------|
| `domain/src/quest.rs` | **Créé** | Types partagés : `QuestStage`, `StageObjective`, `QuestProgress` |
| `domain/src/lib.rs` | Modifié | Re-export du module `quest` |
| `serveur/src/world.rs` | Modifié | `DialogueChoice`, `QuestDialogue`, `QuestStageDef`, champs `stages` + `quest_dialogues` |
| `serveur/src/state.rs` | Modifié | `active_quests` → `quest_progress: Vec<QuestProgress>` |
| `serveur/src/handlers.rs` | Modifié | `handle_talk` stage-aware, `handle_answer`, KillMob tracking, `handle_quests` enrichi |
| `serveur/src/commands.rs` | Modifié | Commande `ANSWER` ajoutée |
| `client_bevy/src/quest.rs` | **Créé** | `QuestTrackerPlugin`, `QuestTracker`, `QuestUpdatedEvent` |
| `client_bevy/src/interaction.rs` | Modifié | Dialogue à choix, `DialogueChoiceBtn`, `render_dialogue_choices`, `handle_choice_click` |
| `client_bevy/src/main.rs` | Modifié | `mod quest`, `QuestTrackerPlugin` enregistré |
| `client_bevy/Cargo.toml` | Modifié | Dépendance `domain` ajoutée |
| `docs/QUEST_SYSTEM.md` | **Créé** | Cette documentation |
