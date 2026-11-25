ALTER TABLE agent ADD COLUMN update_cron TEXT NOT NULL DEFAULT '0 */6 * * *';
ALTER TABLE agent ADD COLUMN self_update_cron TEXT NOT NULL DEFAULT '0 4 * * *';
