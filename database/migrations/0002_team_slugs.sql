-- URL identifiers are persistent display metadata; numeric IDs remain the authorization boundary.
ALTER TABLE teams ADD COLUMN slug TEXT CHECK (slug IS NULL OR length(slug) BETWEEN 1 AND 160);
CREATE UNIQUE INDEX teams_slug_unique ON teams(slug);
