-- Your SQL goes here

-- Rename scan.id to scan.scan_id
ALTER TABLE scan RENAME COLUMN id TO scan_id;
