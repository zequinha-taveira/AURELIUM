-- 007_add_indexes_and_performance.sql
-- Performance indexes and useful views

-- Indexes on goals
CREATE INDEX IF NOT EXISTS idx_goals_status ON goals(status);
CREATE INDEX IF NOT EXISTS idx_goals_created ON goals(created_at DESC);

-- Indexes on missions
CREATE INDEX IF NOT EXISTS idx_missions_goal ON missions(goal_id);
CREATE INDEX IF NOT EXISTS idx_missions_status ON missions(status);
CREATE INDEX IF NOT EXISTS idx_missions_priority ON missions(priority);

-- Indexes on tasks
CREATE INDEX IF NOT EXISTS idx_tasks_mission ON tasks(mission_id);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_agent_type ON tasks(agent_type);

-- Indexes on genome variants
CREATE INDEX IF NOT EXISTS idx_variants_parent ON genome_variants(parent_id);
CREATE INDEX IF NOT EXISTS idx_variants_status ON genome_variants(status);

-- Indexes on telemetry
CREATE INDEX IF NOT EXISTS idx_telemetry_variant ON variant_telemetry(variant_id);
CREATE INDEX IF NOT EXISTS idx_telemetry_created ON variant_telemetry(created_at DESC);

-- Useful views
CREATE OR REPLACE VIEW goal_summary AS
SELECT
    g.id AS goal_id,
    g.raw_input,
    g.status AS goal_status,
    g.created_at,
    COUNT(m.id) AS mission_count,
    COUNT(CASE WHEN m.status = 'completed' THEN 1 END) AS completed_missions
FROM goals g
LEFT JOIN missions m ON m.goal_id = g.id
GROUP BY g.id, g.raw_input, g.status, g.created_at;

CREATE OR REPLACE VIEW system_dashboard AS
SELECT
    (SELECT COUNT(*) FROM goals) AS total_goals,
    (SELECT COUNT(*) FROM goals WHERE status = 'pending') AS pending_goals,
    (SELECT COUNT(*) FROM goals WHERE status = 'completed') AS completed_goals,
    (SELECT COUNT(*) FROM goals WHERE status = 'failed') AS failed_goals,
    (SELECT COUNT(*) FROM missions) AS total_missions,
    (SELECT COUNT(*) FROM tasks) AS total_tasks,
    (SELECT COUNT(*) FROM capabilities) AS total_capabilities,
    (SELECT COUNT(*) FROM genome_variants WHERE status = 'active') AS active_variants;
