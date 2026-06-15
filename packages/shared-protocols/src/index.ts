// ============================================================================
// AURELIUM — Shared Protocols
// Inter-agent communication protocols for the Cognitive Workforce
// ============================================================================

import { AgentType, Priority, HealthStatus } from '@aurelium/shared-types';

// ---------------------------------------------------------------------------
// Agent Lifecycle Protocol
// ---------------------------------------------------------------------------

/**
 * Protocol for agent registration and discovery.
 * Agents must register on startup and send heartbeats periodically.
 */
export interface AgentLifecycleProtocol {
  /** NATS subject for registration */
  registerSubject: string;
  /** NATS subject for heartbeats */
  heartbeatSubject: string;
  /** NATS subject for deregistration */
  deregisterSubject: string;
  /** Heartbeat interval in milliseconds */
  heartbeatIntervalMs: number;
  /** Number of missed heartbeats before agent is considered dead */
  maxMissedHeartbeats: number;
}

export const AGENT_LIFECYCLE: AgentLifecycleProtocol = {
  registerSubject: 'agent.registered',
  heartbeatSubject: 'agent.heartbeat',
  deregisterSubject: 'agent.deregistered',
  heartbeatIntervalMs: 10_000,
  maxMissedHeartbeats: 3,
};

// ---------------------------------------------------------------------------
// Task Assignment Protocol
// ---------------------------------------------------------------------------

/**
 * Protocol for assigning tasks to agents.
 * The coordination engine publishes task.assigned,
 * agents subscribe to their agent-type-specific subject.
 */
export interface TaskAssignmentProtocol {
  /** Subject pattern: task.assign.{agentType} */
  assignSubjectPattern: string;
  /** Subject for task acceptance */
  acceptSubject: string;
  /** Subject for task rejection */
  rejectSubject: string;
  /** Maximum time in ms an agent has to accept/reject */
  acceptTimeoutMs: number;
}

export const TASK_ASSIGNMENT: TaskAssignmentProtocol = {
  assignSubjectPattern: 'task.assign.{agentType}',
  acceptSubject: 'task.accepted',
  rejectSubject: 'task.rejected',
  acceptTimeoutMs: 30_000,
};

/** Resolve the assignment subject for a specific agent type */
export function getAssignSubject(agentType: AgentType): string {
  return TASK_ASSIGNMENT.assignSubjectPattern.replace('{agentType}', agentType);
}

// ---------------------------------------------------------------------------
// Review Protocol
// ---------------------------------------------------------------------------

/**
 * Protocol for code review and approval between agents.
 * Producing agents submit output for review; reviewing agents approve/reject.
 */
export interface ReviewProtocol {
  /** Subject for requesting review */
  requestSubject: string;
  /** Subject for approval */
  approveSubject: string;
  /** Subject for rejection */
  rejectSubject: string;
  /** Maximum time in ms for a reviewer to respond */
  reviewTimeoutMs: number;
  /** Whether security review is mandatory */
  securityReviewRequired: boolean;
}

export const REVIEW_PROTOCOL: ReviewProtocol = {
  requestSubject: 'task.review.requested',
  approveSubject: 'task.review.approved',
  rejectSubject: 'task.review.rejected',
  reviewTimeoutMs: 120_000,
  securityReviewRequired: true,
};

// ---------------------------------------------------------------------------
// Collaboration Protocol
// ---------------------------------------------------------------------------

/**
 * Protocol for agent-to-agent collaboration.
 * Agents can request help, delegate sub-tasks, or share context.
 */
export interface CollaborationMessage {
  /** Unique message ID */
  messageId: string;
  /** Sending agent ID */
  fromAgent: string;
  /** Target agent ID or broadcast */
  toAgent: string | '*';
  /** Type of collaboration request */
  type: CollaborationType;
  /** Message payload */
  payload: Record<string, unknown>;
  /** Priority of the request */
  priority: Priority;
  /** Correlation ID for tracking conversation threads */
  correlationId: string;
  /** Timestamp */
  timestamp: string;
}

export type CollaborationType =
  | 'help_request'      // "I need help with X"
  | 'delegation'        // "Please handle sub-task Y"
  | 'context_share'     // "Here's context you might need"
  | 'review_request'    // "Please review my output"
  | 'consensus_vote'    // "Vote on decision Z"
  | 'status_update'     // "Here's my progress"
  | 'escalation';       // "This needs human attention"

export const COLLABORATION_SUBJECTS = {
  /** Direct message: collab.direct.{agentId} */
  direct: (agentId: string) => `collab.direct.${agentId}`,
  /** Broadcast to all agents */
  broadcast: 'collab.broadcast',
  /** Broadcast to agents of a specific type: collab.type.{agentType} */
  byType: (agentType: AgentType) => `collab.type.${agentType}`,
};

// ---------------------------------------------------------------------------
// Consensus Protocol
// ---------------------------------------------------------------------------

/**
 * Protocol for multi-agent decision-making.
 * Used when multiple agents need to agree on an architecture
 * decision, technology choice, or trade-off.
 */
export interface ConsensusProposal {
  /** Unique proposal ID */
  proposalId: string;
  /** Agent that proposed */
  proposedBy: string;
  /** Description of the decision */
  description: string;
  /** Options to vote on */
  options: ConsensusOption[];
  /** Minimum number of votes required */
  quorum: number;
  /** Deadline for voting */
  deadline: string;
  /** Current status */
  status: 'open' | 'closed' | 'approved' | 'rejected';
}

export interface ConsensusOption {
  id: string;
  label: string;
  description: string;
}

export interface ConsensusVote {
  proposalId: string;
  voterId: string;
  voterType: AgentType;
  selectedOptionId: string;
  confidence: number; // 0.0 - 1.0
  reasoning: string;
  timestamp: string;
}

export const CONSENSUS_SUBJECTS = {
  propose: 'consensus.propose',
  vote: 'consensus.vote',
  result: 'consensus.result',
};

// ---------------------------------------------------------------------------
// Escalation Protocol
// ---------------------------------------------------------------------------

/**
 * Protocol for escalating decisions to human operators.
 * Agents should escalate when confidence is low, stakes are high,
 * or consensus cannot be reached.
 */
export interface EscalationRequest {
  escalationId: string;
  agentId: string;
  agentType: AgentType;
  reason: EscalationReason;
  context: Record<string, unknown>;
  suggestedActions: string[];
  urgency: Priority;
  timestamp: string;
}

export type EscalationReason =
  | 'low_confidence'
  | 'high_risk_change'
  | 'consensus_deadlock'
  | 'security_concern'
  | 'budget_exceeded'
  | 'unknown_domain'
  | 'error_threshold_exceeded';

export const ESCALATION_SUBJECTS = {
  request: 'escalation.request',
  response: 'escalation.response',
  acknowledged: 'escalation.acknowledged',
};

// ---------------------------------------------------------------------------
// Agent Capability Declaration
// ---------------------------------------------------------------------------

/**
 * Capability declaration that agents broadcast on registration.
 * The coordination engine uses this to match tasks to agents.
 */
export interface AgentCapabilityDeclaration {
  agentId: string;
  agentType: AgentType;
  /** List of capability tags (e.g., 'rust', 'api-design', 'sql') */
  capabilities: string[];
  /** Maximum concurrent tasks this agent can handle */
  maxConcurrency: number;
  /** Languages/tech this agent specializes in */
  specializations: string[];
  /** Current load factor (0.0 = idle, 1.0 = fully loaded) */
  loadFactor: number;
}
