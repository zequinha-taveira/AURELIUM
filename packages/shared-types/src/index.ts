// ============================================================================
// AURELIUM — Shared Types
// Core type definitions used across the entire Living Software Fabric
// ============================================================================

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/** Status of a Goal submitted to the Intent Operating System */
export enum GoalStatus {
  PENDING = 'pending',
  PROCESSING = 'processing',
  COMPLETED = 'completed',
  FAILED = 'failed',
  CANCELLED = 'cancelled',
}

/** Priority level for missions, tasks, and capabilities */
export enum Priority {
  CRITICAL = 'critical',
  HIGH = 'high',
  MEDIUM = 'medium',
  LOW = 'low',
}

/** Status of a Mission derived from a Goal */
export enum MissionStatus {
  PENDING = 'pending',
  ASSIGNED = 'assigned',
  IN_PROGRESS = 'in_progress',
  REVIEWING = 'reviewing',
  COMPLETED = 'completed',
  FAILED = 'failed',
  CANCELLED = 'cancelled',
}

/** Status of a Task assigned to an Agent */
export enum TaskStatus {
  PENDING = 'pending',
  ACTIVE = 'active',
  REVIEWING = 'reviewing',
  COMPLETED = 'completed',
  FAILED = 'failed',
  BLOCKED = 'blocked',
}

/** Types of specialized agents in the Cognitive Workforce */
export enum AgentType {
  ARCHITECT = 'architect',
  BACKEND = 'backend',
  FRONTEND = 'frontend',
  SECURITY = 'security',
  DEVOPS = 'devops',
  DATABASE = 'database',
  RESEARCH = 'research',
  OBSERVABILITY = 'observability',
  OPTIMIZATION = 'optimization',
  ECONOMICS = 'economics',
}

/** Health status for system components */
export enum HealthStatus {
  HEALTHY = 'healthy',
  DEGRADED = 'degraded',
  UNHEALTHY = 'unhealthy',
  UNKNOWN = 'unknown',
}

/** Status of a genome variant in the Evolution Engine */
export enum VariantStatus {
  ACTIVE = 'active',
  TESTING = 'testing',
  PROMOTED = 'promoted',
  RETIRED = 'retired',
  EXTINCT = 'extinct',
}

/** Lifecycle phase of a capability species */
export enum SpeciesLifecycle {
  EMBRYO = 'embryo',
  JUVENILE = 'juvenile',
  MATURE = 'mature',
  DECLINING = 'declining',
  EXTINCT = 'extinct',
}

// ---------------------------------------------------------------------------
// Core Entities
// ---------------------------------------------------------------------------

/** A high-level objective submitted by a human */
export interface Goal {
  id: string;
  rawInput: string;
  status: GoalStatus;
  tenantId?: string;
  metadata?: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
}

/** A structured mission decomposed from a Goal */
export interface Mission {
  id: string;
  goalId: string;
  title: string;
  description: string;
  priority: Priority;
  status: MissionStatus;
  assignedAgents?: string[];
  dependencies?: string[];
  createdAt: string;
}

/** A concrete task assigned to a specific Agent */
export interface Task {
  id: string;
  missionId: string;
  agentType: AgentType;
  title: string;
  description: string;
  status: TaskStatus;
  output?: string;
  securityApproval: boolean;
  createdAt: string;
  updatedAt: string;
}

// ---------------------------------------------------------------------------
// Agent Types
// ---------------------------------------------------------------------------

/** Registration information for an Agent */
export interface AgentInfo {
  id: string;
  type: AgentType;
  version: string;
  capabilities: string[];
  status: HealthStatus;
  lastHeartbeat: string;
  metadata?: Record<string, unknown>;
}

/** Heartbeat message sent by agents */
export interface AgentHeartbeat {
  agentId: string;
  agentType: AgentType;
  status: HealthStatus;
  activeTaskCount: number;
  cpuUsage?: number;
  memoryUsage?: number;
  timestamp: string;
}

// ---------------------------------------------------------------------------
// Genome / Capability Types
// ---------------------------------------------------------------------------

/** Semantic DNA of a capability */
export interface CapabilityGenome {
  id: string;
  name: string;
  version: string;
  description: string;
  behaviors: GenomeBehavior[];
  metrics: GenomeMetric[];
  dependencies: string[];
  constraints: GenomeConstraint[];
  rawYaml: string;
  createdAt: string;
  updatedAt: string;
}

/** A behavior specification within a genome */
export interface GenomeBehavior {
  name: string;
  trigger: string;
  action: string;
  expectedOutcome: string;
}

/** A metric tracked for a capability */
export interface GenomeMetric {
  name: string;
  type: 'counter' | 'gauge' | 'histogram';
  unit: string;
  target?: number;
}

/** A constraint that limits mutation/evolution */
export interface GenomeConstraint {
  field: string;
  operator: 'eq' | 'neq' | 'gt' | 'lt' | 'gte' | 'lte' | 'in' | 'regex';
  value: unknown;
  description: string;
}

// ---------------------------------------------------------------------------
// Evolution Types
// ---------------------------------------------------------------------------

/** A mutated variant of a capability genome */
export interface GenomeVariant {
  id: string;
  parentId: string;
  version: string;
  rawYaml: string;
  status: VariantStatus;
  fitnessScore?: number;
  createdAt: string;
}

/** Telemetry data for evaluating variant fitness */
export interface VariantTelemetry {
  id: string;
  variantId: string;
  latencyMs: number;
  errorRate: number;
  successRate: number;
  throughput?: number;
  costPerCall?: number;
  createdAt: string;
}

// ---------------------------------------------------------------------------
// System / Infrastructure Types
// ---------------------------------------------------------------------------

/** Health check response from the API Gateway */
export interface HealthResponse {
  status: HealthStatus;
  database: HealthStatus;
  nats: HealthStatus;
  neo4j?: HealthStatus;
  qdrant?: HealthStatus;
  uptime: number;
  version: string;
}

/** Tenant context for multi-tenancy */
export interface TenantContext {
  tenantId: string;
  organizationName: string;
  plan: 'free' | 'starter' | 'professional' | 'enterprise';
  quotas: TenantQuotas;
}

/** Resource quotas per tenant */
export interface TenantQuotas {
  maxGoalsPerDay: number;
  maxAgents: number;
  maxGenomes: number;
  maxSimulationsPerDay: number;
  maxStorageBytes: number;
}

// ---------------------------------------------------------------------------
// API Request / Response Types
// ---------------------------------------------------------------------------

/** Request to submit a new goal */
export interface IntentRequest {
  goal: string;
  context?: Record<string, unknown>;
  priority?: Priority;
  constraints?: string[];
}

/** Response from goal submission */
export interface IntentResponse {
  status: 'accepted' | 'rejected';
  goalId?: string;
  message: string;
}

/** Paginated list response wrapper */
export interface PaginatedResponse<T> {
  data: T[];
  total: number;
  page: number;
  pageSize: number;
  hasMore: boolean;
}

/** Standard error response */
export interface ErrorResponse {
  error: string;
  code: string;
  details?: Record<string, unknown>;
  timestamp: string;
}
