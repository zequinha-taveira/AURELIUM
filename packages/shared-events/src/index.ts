// ============================================================================
// AURELIUM — Shared Events
// NATS event contracts for the Living Software Fabric event bus
// ============================================================================

import {
  Priority,
  GoalStatus,
  MissionStatus,
  TaskStatus,
  AgentType,
  HealthStatus,
  VariantStatus,
} from '@aurelium/shared-types';

// ---------------------------------------------------------------------------
// NATS Subject Constants
// ---------------------------------------------------------------------------

/** All NATS subject strings used across the system */
export const Subjects = {
  // Intent / Goal events
  INTENT_RECEIVED: 'intent.received',
  INTENT_DECOMPOSE: 'intent.decompose',
  INTENT_FAILED: 'intent.failed',

  // Mission events
  MISSION_GENERATED: 'mission.generated',
  MISSION_ASSIGNED: 'mission.assigned',
  MISSION_COMPLETED: 'mission.completed',
  MISSION_FAILED: 'mission.failed',

  // Task events
  TASK_CREATED: 'task.created',
  TASK_ASSIGNED: 'task.assigned',
  TASK_STARTED: 'task.started',
  TASK_COMPLETED: 'task.completed',
  TASK_FAILED: 'task.failed',
  TASK_REVIEW_REQUESTED: 'task.review.requested',
  TASK_REVIEW_APPROVED: 'task.review.approved',
  TASK_REVIEW_REJECTED: 'task.review.rejected',

  // Agent events
  AGENT_REGISTERED: 'agent.registered',
  AGENT_HEARTBEAT: 'agent.heartbeat',
  AGENT_DEREGISTERED: 'agent.deregistered',
  AGENT_CAPABILITY_UPDATED: 'agent.capability.updated',

  // Genome / Capability events
  GENOME_CREATED: 'genome.created',
  GENOME_UPDATED: 'genome.updated',
  GENOME_DEPRECATED: 'genome.deprecated',

  // Evolution events
  EVOLUTION_MUTATION_CREATED: 'evolution.mutation.created',
  EVOLUTION_VARIANT_TESTED: 'evolution.variant.tested',
  EVOLUTION_VARIANT_PROMOTED: 'evolution.variant.promoted',
  EVOLUTION_VARIANT_RETIRED: 'evolution.variant.retired',
  EVOLUTION_CYCLE_COMPLETED: 'evolution.cycle.completed',

  // Simulation events
  SIMULATION_REQUESTED: 'simulation.requested',
  SIMULATION_STARTED: 'simulation.started',
  SIMULATION_COMPLETED: 'simulation.completed',
  SIMULATION_FAILED: 'simulation.failed',

  // Market events
  MARKET_AUCTION_OPENED: 'market.auction.opened',
  MARKET_BID_PLACED: 'market.bid.placed',
  MARKET_AUCTION_CLOSED: 'market.auction.closed',

  // System events
  SYSTEM_HEALTH: 'system.health',
  SYSTEM_ALERT: 'system.alert',
  SYSTEM_METRICS: 'system.metrics',
} as const;

export type Subject = typeof Subjects[keyof typeof Subjects];

// ---------------------------------------------------------------------------
// Event Base
// ---------------------------------------------------------------------------

/** Base event envelope — all NATS messages follow this structure */
export interface AureliumEvent<T = unknown> {
  /** Unique event ID */
  eventId: string;
  /** NATS subject */
  subject: Subject;
  /** ISO 8601 timestamp */
  timestamp: string;
  /** Tenant ID for multi-tenancy isolation */
  tenantId?: string;
  /** Correlation ID for distributed tracing */
  correlationId?: string;
  /** Source service that emitted this event */
  source: string;
  /** Event-specific payload */
  payload: T;
}

// ---------------------------------------------------------------------------
// Intent Events
// ---------------------------------------------------------------------------

/** Payload for intent.received */
export interface IntentReceivedPayload {
  goal: string;
  context?: Record<string, unknown>;
  priority?: Priority;
}

/** Payload for intent.decompose (NATS Request-Reply) */
export interface IntentDecomposeRequest {
  goalId: string;
  goal: string;
  context?: Record<string, unknown>;
}

/** Response for intent.decompose */
export interface IntentDecomposeResponse {
  missions: DecomposedMission[];
}

export interface DecomposedMission {
  title: string;
  description: string;
  priority: Priority;
}

/** Payload for intent.failed */
export interface IntentFailedPayload {
  goalId: string;
  reason: string;
  error?: string;
}

// ---------------------------------------------------------------------------
// Mission Events
// ---------------------------------------------------------------------------

/** Payload for mission.generated */
export interface MissionGeneratedPayload {
  goalId: string;
  missions: DecomposedMission[];
}

/** Payload for mission.assigned */
export interface MissionAssignedPayload {
  missionId: string;
  goalId: string;
  assignedAgents: string[];
}

/** Payload for mission.completed */
export interface MissionCompletedPayload {
  missionId: string;
  goalId: string;
  results: Record<string, unknown>;
}

/** Payload for mission.failed */
export interface MissionFailedPayload {
  missionId: string;
  goalId: string;
  reason: string;
}

// ---------------------------------------------------------------------------
// Task Events
// ---------------------------------------------------------------------------

/** Payload for task.created / task.assigned */
export interface TaskCreatedPayload {
  taskId: string;
  missionId: string;
  agentType: AgentType;
  title: string;
  description: string;
}

/** Payload for task.completed */
export interface TaskCompletedPayload {
  taskId: string;
  missionId: string;
  agentType: AgentType;
  output: string;
}

/** Payload for task.failed */
export interface TaskFailedPayload {
  taskId: string;
  missionId: string;
  agentType: AgentType;
  reason: string;
  error?: string;
}

/** Payload for task.review.requested */
export interface TaskReviewRequestedPayload {
  taskId: string;
  missionId: string;
  reviewerAgentType: AgentType;
  output: string;
}

/** Payload for task.review.approved / rejected */
export interface TaskReviewResultPayload {
  taskId: string;
  missionId: string;
  reviewerAgentType: AgentType;
  approved: boolean;
  comments?: string;
}

// ---------------------------------------------------------------------------
// Agent Events
// ---------------------------------------------------------------------------

/** Payload for agent.registered */
export interface AgentRegisteredPayload {
  agentId: string;
  agentType: AgentType;
  version: string;
  capabilities: string[];
}

/** Payload for agent.heartbeat */
export interface AgentHeartbeatPayload {
  agentId: string;
  agentType: AgentType;
  status: HealthStatus;
  activeTaskCount: number;
  cpuUsage?: number;
  memoryUsage?: number;
}

/** Payload for agent.deregistered */
export interface AgentDeregisteredPayload {
  agentId: string;
  agentType: AgentType;
  reason: string;
}

// ---------------------------------------------------------------------------
// Genome Events
// ---------------------------------------------------------------------------

/** Payload for genome.created / genome.updated */
export interface GenomeEventPayload {
  capabilityId: string;
  name: string;
  version: string;
  description: string;
}

// ---------------------------------------------------------------------------
// Evolution Events
// ---------------------------------------------------------------------------

/** Payload for evolution.mutation.created */
export interface MutationCreatedPayload {
  variantId: string;
  parentId: string;
  mutationType: 'parametric' | 'structural' | 'behavioral';
  version: string;
}

/** Payload for evolution.variant.tested */
export interface VariantTestedPayload {
  variantId: string;
  parentId: string;
  fitnessScore: number;
  latencyMs: number;
  errorRate: number;
  successRate: number;
}

/** Payload for evolution.variant.promoted / retired */
export interface VariantLifecyclePayload {
  variantId: string;
  parentId: string;
  previousStatus: VariantStatus;
  newStatus: VariantStatus;
  reason: string;
}

/** Payload for evolution.cycle.completed */
export interface EvolutionCycleCompletedPayload {
  cycleId: string;
  generation: number;
  totalVariants: number;
  promoted: number;
  retired: number;
  avgFitness: number;
}

// ---------------------------------------------------------------------------
// Simulation Events
// ---------------------------------------------------------------------------

/** Payload for simulation.requested */
export interface SimulationRequestedPayload {
  simulationId: string;
  scenarioType: string;
  parameters: Record<string, unknown>;
  requestedBy: string;
}

/** Payload for simulation.completed */
export interface SimulationCompletedPayload {
  simulationId: string;
  scenarioType: string;
  totalFutures: number;
  bestOutcome: Record<string, unknown>;
  worstOutcome: Record<string, unknown>;
  recommendation: string;
  durationMs: number;
}

// ---------------------------------------------------------------------------
// System Events
// ---------------------------------------------------------------------------

/** Payload for system.health */
export interface SystemHealthPayload {
  services: Record<string, HealthStatus>;
  overallStatus: HealthStatus;
}

/** Payload for system.alert */
export interface SystemAlertPayload {
  severity: 'info' | 'warning' | 'critical';
  service: string;
  message: string;
  details?: Record<string, unknown>;
}

/** Payload for system.metrics */
export interface SystemMetricsPayload {
  goalsProcessed: number;
  activeMissions: number;
  activeTasks: number;
  activeAgents: number;
  avgLatencyMs: number;
  errorRate: number;
  periodSeconds: number;
}
