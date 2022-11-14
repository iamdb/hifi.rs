CREATE TABLE IF NOT EXISTS "audio_quality" (
	"quality_id"	INTEGER NOT NULL DEFAULT 5 UNIQUE,
	"name"	INTEGER NOT NULL DEFAULT mp3,
	PRIMARY KEY("quality_id")
);
CREATE TABLE IF NOT EXISTS "player_queue" (
	"position"	INTEGER NOT NULL,
	"track_id"	INTEGER NOT NULL,
	"is_album"	NUMERIC NOT NULL DEFAULT 0,
	"is_playlist"	INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS "state" (
	"key"	TEXT NOT NULL DEFAULT NULL UNIQUE,
	"value"	BLOB NOT NULL
);
CREATE TABLE IF NOT EXISTS "config" (
	"username"	TEXT UNIQUE,
	"password"	TEXT,
	"default_quality"	INTEGER DEFAULT 5,
	"user_token"	TEXT,
	"active_secret"	TEXT,
	"app_id"	TEXT,
	FOREIGN KEY("default_quality") REFERENCES "audio_quality"("quality_id")
);
INSERT INTO "audio_quality" VALUES (5,'mp3');
INSERT INTO "audio_quality" VALUES (6,'cd');
INSERT INTO "audio_quality" VALUES (7,'hifi96');
INSERT INTO "audio_quality" VALUES (27,'mp3');
