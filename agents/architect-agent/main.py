"""
AURELIUM — Architect Agent
===========================
Decomposes high-level human goals into structured, actionable missions.
Supports multi-model fallback: Gemini → OpenAI → Mock.
"""

import asyncio
import json
import logging
import os
import sys
import time
from typing import Optional

import aiohttp
from nats.aio.client import Client as NATSClient

# ============================================================================
# Configuration
# ============================================================================

NATS_URL = os.getenv("NATS_URL", "nats://localhost:4222")
GEMINI_API_KEY = os.getenv("GEMINI_API_KEY")
OPENAI_API_KEY = os.getenv("OPENAI_API_KEY")
LOG_LEVEL = os.getenv("LOG_LEVEL", "INFO").upper()

# Configure structured logging
logging.basicConfig(
    level=getattr(logging, LOG_LEVEL, logging.INFO),
    format="%(asctime)s | %(levelname)-8s | %(name)s | %(message)s",
    datefmt="%Y-%m-%dT%H:%M:%S",
    stream=sys.stdout,
)
logger = logging.getLogger("aurelium.architect-agent")

# ============================================================================
# Metrics
# ============================================================================

metrics = {
    "goals_decomposed": 0,
    "goals_failed": 0,
    "gemini_calls": 0,
    "openai_calls": 0,
    "mock_calls": 0,
    "avg_latency_ms": 0.0,
    "total_latency_ms": 0.0,
}

# ============================================================================
# Mission Schema Validation
# ============================================================================

VALID_PRIORITIES = {"critical", "high", "medium", "low"}


def validate_missions(missions: list) -> list:
    """Validate and normalize mission structure."""
    validated = []
    for m in missions:
        if not isinstance(m, dict):
            continue
        title = str(m.get("title", "")).strip()
        if not title:
            continue
        validated.append({
            "title": title[:255],
            "description": str(m.get("description", "")).strip()[:2000],
            "priority": m.get("priority", "medium").lower()
            if m.get("priority", "").lower() in VALID_PRIORITIES
            else "medium",
        })
    return validated


# ============================================================================
# Decomposition Strategies
# ============================================================================

SYSTEM_PROMPT = (
    "You are the AURELIUM Architect Agent, part of a Living Software Fabric. "
    "Your role is to decompose high-level human objectives into concrete, "
    "actionable missions. Each mission should be specific, measurable, and "
    "achievable by a specialized AI agent.\n\n"
    "Rules:\n"
    "1. Generate 2-6 missions per goal.\n"
    "2. Each mission must have: title, description, priority (critical/high/medium/low).\n"
    "3. Missions should be ordered by dependency (independent tasks first).\n"
    "4. Be concrete — avoid vague descriptions.\n"
    "5. Consider security, performance, and cost implications.\n\n"
    "Respond ONLY with a JSON object matching this schema:\n"
    '{"missions": [{"title": "string", "description": "string", "priority": "high"|"medium"|"low"}]}\n'
    "Do not include markdown blocks, explanations, or anything else — just raw JSON."
)


async def decompose_goal_gemini(goal: str, context: Optional[dict] = None) -> list:
    """Decompose goal using Google Gemini API."""
    url = f"https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={GEMINI_API_KEY}"
    headers = {"Content-Type": "application/json"}

    user_prompt = f"Goal: '{goal}'"
    if context:
        user_prompt += f"\nAdditional Context: {json.dumps(context)}"

    payload = {
        "contents": [
            {"role": "user", "parts": [{"text": SYSTEM_PROMPT + "\n\n" + user_prompt}]}
        ],
        "generationConfig": {
            "temperature": 0.3,
            "topP": 0.8,
            "maxOutputTokens": 2048,
        },
    }

    async with aiohttp.ClientSession() as session:
        async with session.post(url, headers=headers, json=payload, timeout=aiohttp.ClientTimeout(total=30)) as resp:
            if resp.status == 200:
                result = await resp.json()
                text = result["candidates"][0]["content"]["parts"][0]["text"].strip()
                # Clean up potential markdown wrapper
                if text.startswith("```json"):
                    text = text[7:]
                if text.startswith("```"):
                    text = text[3:]
                if text.endswith("```"):
                    text = text[:-3]
                data = json.loads(text.strip())
                metrics["gemini_calls"] += 1
                return data.get("missions", [])
            else:
                error_body = await resp.text()
                logger.error(f"Gemini API returned status {resp.status}: {error_body[:500]}")
                raise RuntimeError(f"Gemini API error: {resp.status}")


async def decompose_goal_openai(goal: str, context: Optional[dict] = None) -> list:
    """Decompose goal using OpenAI API."""
    url = "https://api.openai.com/v1/chat/completions"
    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {OPENAI_API_KEY}",
    }

    user_prompt = f"Goal: {goal}"
    if context:
        user_prompt += f"\nAdditional Context: {json.dumps(context)}"

    payload = {
        "model": "gpt-4o-mini",
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": user_prompt},
        ],
        "response_format": {"type": "json_object"},
        "temperature": 0.3,
        "max_tokens": 2048,
    }

    async with aiohttp.ClientSession() as session:
        async with session.post(url, headers=headers, json=payload, timeout=aiohttp.ClientTimeout(total=30)) as resp:
            if resp.status == 200:
                result = await resp.json()
                text = result["choices"][0]["message"]["content"].strip()
                data = json.loads(text)
                metrics["openai_calls"] += 1
                return data.get("missions", [])
            else:
                error_body = await resp.text()
                logger.error(f"OpenAI API returned status {resp.status}: {error_body[:500]}")
                raise RuntimeError(f"OpenAI API error: {resp.status}")


async def decompose_goal_mock(goal: str, context: Optional[dict] = None) -> list:
    """Intelligent mock decomposition with pattern matching."""
    metrics["mock_calls"] += 1
    goal_lower = goal.lower()

    # Revenue / Growth patterns
    if any(kw in goal_lower for kw in ["revenue", "profit", "growth", "sales", "monetize"]):
        return [
            {"title": "Analyze current revenue streams", "description": "Map all existing revenue sources, identify top performers and underperformers, and calculate contribution margins.", "priority": "high"},
            {"title": "Optimize sales conversion funnel", "description": "Identify and reduce friction points in the customer acquisition journey using behavioral analytics.", "priority": "high"},
            {"title": "Reduce customer churn", "description": "Implement automated renewal outreach, feedback loops for departing customers, and predictive churn modeling.", "priority": "medium"},
            {"title": "Improve checkout conversion rate", "description": "Redesign checkout forms, add trust signals, and implement A/B testing for cart abandonment reduction.", "priority": "medium"},
        ]

    # Cost / Efficiency patterns
    if any(kw in goal_lower for kw in ["cost", "expense", "support", "reduce", "optimize", "efficiency"]):
        return [
            {"title": "Audit current cost structure", "description": "Analyze all operational costs, identify top expense categories, and benchmark against industry standards.", "priority": "high"},
            {"title": "Automate repetitive workflows", "description": "Identify manual processes that can be automated using AI agents and workflow engines.", "priority": "high"},
            {"title": "Optimize infrastructure costs", "description": "Right-size cloud resources, implement auto-scaling, and evaluate reserved capacity discounts.", "priority": "medium"},
            {"title": "Implement monitoring and alerting", "description": "Set up cost anomaly detection and automated alerts for budget threshold breaches.", "priority": "low"},
        ]

    # Security patterns
    if any(kw in goal_lower for kw in ["security", "secure", "protect", "compliance", "vulnerability"]):
        return [
            {"title": "Security posture assessment", "description": "Conduct comprehensive vulnerability scan, dependency audit, and configuration review.", "priority": "critical"},
            {"title": "Implement zero-trust architecture", "description": "Deploy identity-aware proxy, enforce least-privilege access, and add network segmentation.", "priority": "high"},
            {"title": "Set up automated security testing", "description": "Integrate SAST, DAST, and dependency scanning into CI/CD pipeline.", "priority": "high"},
            {"title": "Create incident response plan", "description": "Document response procedures, set up alerting, and conduct tabletop exercises.", "priority": "medium"},
        ]

    # Performance patterns
    if any(kw in goal_lower for kw in ["performance", "speed", "fast", "latency", "scale", "scalable"]):
        return [
            {"title": "Performance baseline assessment", "description": "Establish current performance metrics, identify bottlenecks, and set SLO targets.", "priority": "high"},
            {"title": "Implement caching strategy", "description": "Add application-level caching, CDN configuration, and database query optimization.", "priority": "high"},
            {"title": "Database optimization", "description": "Analyze slow queries, add missing indexes, and optimize schema for read/write patterns.", "priority": "medium"},
            {"title": "Load testing and capacity planning", "description": "Design load test scenarios, execute under realistic conditions, and project scaling requirements.", "priority": "medium"},
        ]

    # Generic fallback
    return [
        {"title": f"Analyze current state: {goal[:100]}", "description": "Gather telemetry, context, and configurations. Identify key metrics and success criteria.", "priority": "high"},
        {"title": f"Design implementation plan: {goal[:100]}", "description": "Create detailed technical plan with milestones, resource requirements, and risk assessment.", "priority": "high"},
        {"title": f"Execute initial implementation: {goal[:100]}", "description": "Build core functionality with automated tests. Follow iterative delivery approach.", "priority": "medium"},
        {"title": f"Validate and optimize: {goal[:100]}", "description": "Run integration tests, collect metrics, and iterate based on results.", "priority": "low"},
    ]


# ============================================================================
# Multi-Model Fallback Chain
# ============================================================================


async def decompose_goal(goal: str, context: Optional[dict] = None) -> list:
    """
    Decompose a goal using the best available model.
    Fallback chain: Gemini → OpenAI → Mock
    """
    errors = []

    # Try Gemini first
    if GEMINI_API_KEY:
        try:
            logger.info(f"Attempting decomposition with Gemini...")
            missions = await decompose_goal_gemini(goal, context)
            validated = validate_missions(missions)
            if validated:
                return validated
            logger.warning("Gemini returned no valid missions, falling back...")
        except Exception as e:
            logger.warning(f"Gemini failed: {e}")
            errors.append(f"Gemini: {e}")

    # Try OpenAI
    if OPENAI_API_KEY:
        try:
            logger.info(f"Attempting decomposition with OpenAI...")
            missions = await decompose_goal_openai(goal, context)
            validated = validate_missions(missions)
            if validated:
                return validated
            logger.warning("OpenAI returned no valid missions, falling back...")
        except Exception as e:
            logger.warning(f"OpenAI failed: {e}")
            errors.append(f"OpenAI: {e}")

    # Fallback to mock
    if errors:
        logger.warning(f"All LLM providers failed ({errors}), using mock decomposition")
    else:
        logger.info("No LLM API keys configured, using mock decomposition")

    missions = await decompose_goal_mock(goal, context)
    return validate_missions(missions)


# ============================================================================
# NATS Message Handler
# ============================================================================

nc: Optional[NATSClient] = None


async def message_handler(msg):
    """Handle NATS intent decomposition requests."""
    start = time.monotonic()
    data = json.loads(msg.data.decode())
    goal_id = data.get("goal_id", "unknown")
    goal = data.get("goal", "")
    context = data.get("context")

    logger.info(f"[{goal_id}] Decomposing goal: '{goal[:100]}'...")

    try:
        missions = await decompose_goal(goal, context)

        # Update metrics
        elapsed_ms = (time.monotonic() - start) * 1000
        metrics["goals_decomposed"] += 1
        metrics["total_latency_ms"] += elapsed_ms
        metrics["avg_latency_ms"] = (
            metrics["total_latency_ms"] / metrics["goals_decomposed"]
        )

        response = {"missions": missions}
        response_bytes = json.dumps(response).encode()

        if msg.reply:
            await nc.publish(msg.reply, response_bytes)

        logger.info(
            f"[{goal_id}] Responded with {len(missions)} missions "
            f"(latency: {elapsed_ms:.0f}ms)"
        )
    except Exception as e:
        metrics["goals_failed"] += 1
        logger.error(f"[{goal_id}] Error decomposing goal: {e}", exc_info=True)

        # Reply with empty missions on error
        err_response = {"missions": []}
        if msg.reply:
            await nc.publish(msg.reply, json.dumps(err_response).encode())


# ============================================================================
# Metrics Reporter
# ============================================================================


async def report_metrics():
    """Periodically log metrics."""
    while True:
        await asyncio.sleep(60)
        logger.info(
            f"Architect Agent Metrics | "
            f"decomposed={metrics['goals_decomposed']} | "
            f"failed={metrics['goals_failed']} | "
            f"gemini={metrics['gemini_calls']} | "
            f"openai={metrics['openai_calls']} | "
            f"mock={metrics['mock_calls']} | "
            f"avg_latency={metrics['avg_latency_ms']:.0f}ms"
        )


# ============================================================================
# Main
# ============================================================================


async def main():
    global nc
    nc = NATSClient()

    logger.info(f"Connecting to NATS at {NATS_URL}...")
    await nc.connect(NATS_URL)
    logger.info("Connected successfully.")

    # Log provider status
    providers = []
    if GEMINI_API_KEY:
        providers.append("Gemini")
    if OPENAI_API_KEY:
        providers.append("OpenAI")
    providers.append("Mock (fallback)")
    logger.info(f"Available LLM providers: {', '.join(providers)}")

    # Subscribe to intent.decompose
    await nc.subscribe("intent.decompose", cb=message_handler)
    logger.info("Subscribed to 'intent.decompose'. Architect Agent is active.")

    # Start metrics reporter
    asyncio.create_task(report_metrics())

    # Keep agent running
    try:
        while True:
            await asyncio.sleep(1)
    except KeyboardInterrupt:
        logger.info("Shutting down Architect Agent...")
        await nc.close()


if __name__ == "__main__":
    asyncio.run(main())
