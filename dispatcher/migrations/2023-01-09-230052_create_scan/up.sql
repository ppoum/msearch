-- Your SQL goes here

-- Create scan table with needed columns
CREATE TABLE scan (
    id SERIAL PRIMARY KEY,
    ip inet NOT NULL,
    version TEXT,
    online_count INT,
    max_count INT,
    description TEXT,
    favicon TEXT
);
