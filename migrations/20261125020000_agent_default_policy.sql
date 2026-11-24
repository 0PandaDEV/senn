ALTER TABLE agent ADD COLUMN default_policy TEXT NOT NULL DEFAULT 'notify';
UPDATE agent SET default_policy = CASE auto_update WHEN 1 THEN 'auto' ELSE 'notify' END;
