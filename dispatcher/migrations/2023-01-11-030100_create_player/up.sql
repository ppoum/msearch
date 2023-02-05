-- Your SQL goes here

-- Create player table
CREATE TABLE player (
    player_id SERIAL PRIMARY KEY,
    username TEXT NOT NULL,
    player_uuid UUID
);

-- Create many-to-many relation table
CREATE TABLE player_scan (
    player_scan_uuid UUID NOT NULL,
    player_id SERIAL REFERENCES player (player_id) ON UPDATE CASCADE ON DELETE CASCADE,
    scan_id SERIAL REFERENCES scan (scan_id) ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT player_scan_pkey PRIMARY KEY (player_id, scan_id)
);
