CREATE TABLE IF NOT EXISTS "player_state" (
 "rowid" INTEGER NOT NULL UNIQUE,
 "playback_track_id" INTEGER NOT NULL,
 "playback_position" INTEGER NOT NULL,
 "playback_track_index" INTEGER NOT NULL,
 "playback_entity_id" TEXT NOT NULL,
 "playback_entity_type" TEXT NOT NULL DEFAULT 'album',
 PRIMARY KEY("rowid" AUTOINCREMENT),
 FOREIGN KEY("playback_entity_type") REFERENCES "playback_entity"("type")
)
