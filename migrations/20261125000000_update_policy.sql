CREATE TABLE update_policy (
    agent TEXT NOT NULL,
    scope TEXT NOT NULL,
    policy TEXT NOT NULL,
    PRIMARY KEY (agent, scope)
);
