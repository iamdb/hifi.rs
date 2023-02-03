CREATE TABLE IF NOT EXISTS "playback_entity" (
	"type" TEXT NOT NULL DEFAULT 'album',
	PRIMARY KEY("type")
);

INSERT INTO "playback_entity" VALUES ('album');
INSERT INTO "playback_entity" VALUES ('track');
INSERT INTO "playback_entity" VALUES ('playlist');
