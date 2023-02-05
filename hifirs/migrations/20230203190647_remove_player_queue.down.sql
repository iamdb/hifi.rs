CREATE TABLE IF NOT EXISTS "player_queue" (
	"position" INTEGER NOT NULL,
	"track_id" INTEGER NOT NULL,
	"is_album" NUMERIC NOT NULL DEFAULT 0,
	"is_playlist"	INTEGER NOT NULL DEFAULT 0
);

