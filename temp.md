
npcs:
    - id: "npc_vieux_sage"
      name: "Aldous le Borgne"
      role: "quest_giver"
      hp: 50
      dialogue:
        - "Les ruines du château ne sont plus sûres, voyageur..."
        - "Apporte-moi l'Œil de Corbeau, et je guiderai tes pas."
items:
    - id: "item_epee_rouillee"
      name: "Épée Rouillée"
      description: "Une lame usée par le sang et le temps. (+5 ATK)"
      obtainable: true

locations:
    village_square:
      name: "Place d'Ombreval"
      description: "Le centre du village, enveloppé de brume."
      exits:
        north: "ruines_entree"
        east: "taverne_sombre"
        south: "cimetiere"
        west: "route_ouest"
      npcs: ["npc_vieux_sage"]
      items: []

quests:
    - id: "quest_premiere_arme"
      name: "S'armer pour survivre"
      description: |
        Trouve une arme pour te défendre dans la taverne.
      type: "fetch_item"
      target_id: "item_epee_rouillee"
      reward_exp: 50

    - id: "quest_oeil_sage"
      name: "La vision d'Aldous"
      description: |
        Rapporte l'Oeil de Corbeau trouvé dans le marais à Aldous.
      type: "deliver_item"
      target_id: "item_oeil_corbeau"
      giver_id: "npc_vieux_sage"
      reward_item: "item_cle_donjon"
