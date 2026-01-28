-- Add protocol_type column to containers table
ALTER TABLE containers ADD COLUMN protocol_type TEXT NOT NULL DEFAULT 'tcp' CHECK(protocol_type IN ('tcp', 'http'));
