-- P4 fix: events.ts used ISO 'T' format, breaking string range compares.
-- Normalize to 'YYYY-MM-DD HH:MM:SS' like the rest of the schema.
UPDATE events SET ts = substr(replace(ts, 'T', ' '), 1, 19) WHERE ts LIKE '%T%';
