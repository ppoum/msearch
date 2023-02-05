-- This file should undo anything in `up.sql`

-- Undo renaming of scan.id to scan.scan_id
ALTER TABLE scan RENAME COLUMN scan_id TO id;
