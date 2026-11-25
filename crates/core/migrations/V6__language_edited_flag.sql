-- Built-in languages are refreshed from the app's presets on every start, so
-- preset fixes reach existing installs. A language the user has changed is
-- left alone; this flag records that.
ALTER TABLE language ADD COLUMN edited_by_user INTEGER NOT NULL DEFAULT 0;
