-- Position persistée du joueur dans sa salle.
-- NULL = jamais sauvegardée : le client utilise alors le point d'apparition
-- par défaut (SPAWN_POINT), ce qui vaut pour tout nouveau joueur.
ALTER TABLE players ADD COLUMN pos_x REAL;
ALTER TABLE players ADD COLUMN pos_y REAL;
