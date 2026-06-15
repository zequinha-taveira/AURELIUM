// ============================================================================
// AURELIUM — Shared Contracts
// API route contracts and endpoint definitions for the Gateway
// ============================================================================

import {
  Goal,
  Mission,
  Task,
  AgentInfo,
  CapabilityGenome,
  GenomeVariant,
  HealthResponse,
  IntentRequest,
  IntentResponse,
  PaginatedResponse,
  ErrorResponse,
  TenantContext,
  SystemMetrics,
} from '@aurelium/shared-types';

// ---------------------------------------------------------------------------
// API Version
// ---------------------------------------------------------------------------

export const API_VERSION = 'v1';
export const API_BASE = `/api/${API_VERSION}`;

// ---------------------------------------------------------------------------
// Route Definitions
// ---------------------------------------------------------------------------

/** All API routes organized by domain */
export const Routes = {
  // Health & System
  health: '/health',
  metrics: '/metrics',

  // Goals (Intent Operating System)
  goals: {
    list: `${API_BASE}/goals`,
    create: `${API_BASE}/goals`,
    get: (id: string) => `${API_BASE}/goals/${id}`,
    missions: (id: string) => `${API_BASE}/goals/${id}/missions`,
    cancel: (id: string) => `${API_BASE}/goals/${id}/cancel`,
  },

  // Missions
  missions: {
    list: `${API_BASE}/missions`,
    get: (id: string) => `${API_BASE}/missions/${id}`,
    tasks: (id: string) => `${API_BASE}/missions/${id}/tasks`,
    assign: (id: string) => `${API_BASE}/missions/${id}/assign`,
  },

  // Tasks
  tasks: {
    list: `${API_BASE}/tasks`,
    get: (id: string) => `${API_BASE}/tasks/${id}`,
    approve: (id: string) => `${API_BASE}/tasks/${id}/approve`,
    reject: (id: string) => `${API_BASE}/tasks/${id}/reject`,
  },

  // Agents
  agents: {
    list: `${API_BASE}/agents`,
    get: (id: string) => `${API_BASE}/agents/${id}`,
    register: `${API_BASE}/agents/register`,
    heartbeat: `${API_BASE}/agents/heartbeat`,
  },

  // Genomes / Capabilities
  genomes: {
    list: `${API_BASE}/genomes`,
    create: `${API_BASE}/genomes`,
    get: (id: string) => `${API_BASE}/genomes/${id}`,
    variants: (id: string) => `${API_BASE}/genomes/${id}/variants`,
    dependencies: (id: string) => `${API_BASE}/genomes/${id}/dependencies`,
  },

  // Evolution
  evolution: {
    status: `${API_BASE}/evolution/status`,
    cycles: `${API_BASE}/evolution/cycles`,
    variants: `${API_BASE}/evolution/variants`,
    triggerCycle: `${API_BASE}/evolution/trigger`,
  },

  // Simulation
  simulation: {
    list: `${API_BASE}/simulations`,
    create: `${API_BASE}/simulations`,
    get: (id: string) => `${API_BASE}/simulations/${id}`,
    results: (id: string) => `${API_BASE}/simulations/${id}/results`,
  },

  // System
  system: {
    stats: `${API_BASE}/system/stats`,
    config: `${API_BASE}/system/config`,
    events: `${API_BASE}/system/events`,
  },
} as const;

// ---------------------------------------------------------------------------
// Endpoint Contract Definitions
// ---------------------------------------------------------------------------

/**
 * Contract for an API endpoint, defining its HTTP method,
 * path, request body type, query params, and response type.
 */
export interface EndpointContract<
  TRequest = void,
  TResponse = void,
  TParams = void,
  TQuery = void,
> {
  method: 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH';
  path: string;
  description: string;
  auth: boolean;
  request?: TRequest;
  response?: TResponse;
  params?: TParams;
  query?: TQuery;
}

// ---------------------------------------------------------------------------
// Goal Endpoint Contracts
// ---------------------------------------------------------------------------

export type CreateGoalContract = EndpointContract<
  IntentRequest,
  IntentResponse
>;

export type ListGoalsContract = EndpointContract<
  void,
  PaginatedResponse<Goal>,
  void,
  { page?: number; pageSize?: number; status?: string }
>;

export type GetGoalContract = EndpointContract<
  void,
  Goal,
  { id: string }
>;

export type ListGoalMissionsContract = EndpointContract<
  void,
  Mission[],
  { id: string }
>;

// ---------------------------------------------------------------------------
// Mission Endpoint Contracts
// ---------------------------------------------------------------------------

export type ListMissionsContract = EndpointContract<
  void,
  PaginatedResponse<Mission>,
  void,
  { page?: number; pageSize?: number; status?: string; goalId?: string }
>;

export type GetMissionContract = EndpointContract<
  void,
  Mission,
  { id: string }
>;

// ---------------------------------------------------------------------------
// Task Endpoint Contracts
// ---------------------------------------------------------------------------

export type ListTasksContract = EndpointContract<
  void,
  PaginatedResponse<Task>,
  void,
  { page?: number; pageSize?: number; status?: string; agentType?: string }
>;

export type GetTaskContract = EndpointContract<
  void,
  Task,
  { id: string }
>;

// ---------------------------------------------------------------------------
// Agent Endpoint Contracts
// ---------------------------------------------------------------------------

export type ListAgentsContract = EndpointContract<
  void,
  AgentInfo[],
  void,
  { type?: string; status?: string }
>;

export type RegisterAgentContract = EndpointContract<
  { agentType: string; version: string; capabilities: string[] },
  AgentInfo
>;

// ---------------------------------------------------------------------------
// Error Codes
// ---------------------------------------------------------------------------

/** Standardized error codes returned by the API */
export const ErrorCodes = {
  // Client errors
  INVALID_INPUT: 'INVALID_INPUT',
  INPUT_TOO_LONG: 'INPUT_TOO_LONG',
  NOT_FOUND: 'NOT_FOUND',
  UNAUTHORIZED: 'UNAUTHORIZED',
  FORBIDDEN: 'FORBIDDEN',
  RATE_LIMITED: 'RATE_LIMITED',
  CONFLICT: 'CONFLICT',

  // Server errors
  INTERNAL_ERROR: 'INTERNAL_ERROR',
  DB_ERROR: 'DB_ERROR',
  NATS_UNAVAILABLE: 'NATS_UNAVAILABLE',
  SERIALIZATION_ERROR: 'SERIALIZATION_ERROR',
  TIMEOUT: 'TIMEOUT',
  SERVICE_UNAVAILABLE: 'SERVICE_UNAVAILABLE',
} as const;

export type ErrorCode = typeof ErrorCodes[keyof typeof ErrorCodes];
