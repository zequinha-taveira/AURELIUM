-- 004_init_evolution_schema.sql
-- Create genome_variants table
CREATE TABLE IF NOT EXISTS genome_variants (
    id VARCHAR(100) PRIMARY KEY, -- e.g. 'capability.db_query@v1.0.0-mut-1'
    parent_id VARCHAR(100) NOT NULL REFERENCES capabilities(id) ON DELETE CASCADE,
    version VARCHAR(50) NOT NULL,
    raw_yaml TEXT NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'active', -- 'active', 'retired', 'promoted'
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Create variant_telemetry table
CREATE TABLE IF NOT EXISTS variant_telemetry (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    variant_id VARCHAR(100) NOT NULL, -- references capabilities(id) or genome_variants(id)
    latency_ms DOUBLE PRECISION NOT NULL,
    error_rate DOUBLE PRECISION NOT NULL,
    success_rate DOUBLE PRECISION NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);
