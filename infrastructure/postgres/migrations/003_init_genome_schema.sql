-- 003_init_genome_schema.sql
-- Create capabilities table
CREATE TABLE IF NOT EXISTS capabilities (
    id VARCHAR(100) PRIMARY KEY, -- e.g. 'capability.db_query'
    name VARCHAR(255) NOT NULL,
    version VARCHAR(50) NOT NULL,
    description TEXT,
    raw_yaml TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);
