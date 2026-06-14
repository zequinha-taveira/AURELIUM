import asyncio
import json
import os
import aiohttp
from nats.aio.client import Client as NATSClient

NATS_URL = os.getenv("NATS_URL", "nats://localhost:4222")
GEMINI_API_KEY = os.getenv("GEMINI_API_KEY")
OPENAI_API_KEY = os.getenv("OPENAI_API_KEY")


async def decompose_goal_mock(goal: str) -> list:
    """Fall back to high-quality mock missions if no API keys are available."""
    goal_lower = goal.lower()
    if "revenue" in goal_lower or "profit" in goal_lower:
        return [
            {
                "title": "Optimize sales funnel",
                "description": "Analyze and improve friction points in current customer acquisition journey.",
                "priority": "high",
            },
            {
                "title": "Reduce customer churn",
                "description": "Implement automated renewal outreach and feedback loop for departing customers.",
                "priority": "high",
            },
            {
                "title": "Improve checkout conversion rate",
                "description": "Redesign checkout forms to reduce cart abandonment.",
                "priority": "medium",
            },
        ]
    elif "support" in goal_lower or "cost" in goal_lower or "expense" in goal_lower:
        return [
            {
                "title": "Automate FAQs",
                "description": "Deploy an AI-native FAQ assistant to deflect standard ticket categories.",
                "priority": "high",
            },
            {
                "title": "Optimize support workflows",
                "description": "Auto-triage incoming tickets using semantic classification.",
                "priority": "medium",
            },
            {
                "title": "Reduce average response time",
                "description": "Create automated macro templates for high-frequency inquiries.",
                "priority": "low",
            },
        ]
    else:
        return [
            {
                "title": f"Analyze current state for: {goal}",
                "description": "Gather telemetry, context, and configurations associated with the goal.",
                "priority": "high",
            },
            {
                "title": f"Design optimization plan for: {goal}",
                "description": "Establish key performance metrics and structural changes needed.",
                "priority": "medium",
            },
            {
                "title": f"Execute action items for: {goal}",
                "description": "Iteratively roll out changes and monitor impact over time.",
                "priority": "low",
            },
        ]


async def decompose_goal_gemini(goal: str, api_key: str) -> list:
    """Query Gemini API for structured goal decomposition."""
    url = f"https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key={api_key}"
    headers = {"Content-Type": "application/json"}
    prompt = (
        f"You are the AURELIUM Architect Agent. Your goal is to decompose a high-level human objective into a list of structured missions. "
        f"Goal: '{goal}'\n\n"
        f"Respond ONLY with a JSON object matching this schema:\n"
        f"{{\n"
        f"  \"missions\": [\n"
        f"    {{\n"
        f"      \"title\": \"string\",\n"
        f"      \"description\": \"string\",\n"
        f"      \"priority\": \"high\" | \"medium\" | \"low\"\n"
        f"    }}\n"
        f"  ]\n"
        f"}}\n"
        f"Do not include markdown blocks, just raw JSON."
    )
    payload = {"contents": [{"parts": [{"text": prompt}]}]}

    async with aiohttp.ClientSession() as session:
        async with session.post(url, headers=headers, json=payload) as resp:
            if resp.status == 200:
                result = await resp.json()
                text = result["candidates"][0]["content"]["parts"][0]["text"].strip()
                # Clean up potential markdown wrapper
                if text.startswith("```json"):
                    text = text[7:]
                if text.endswith("```"):
                    text = text[:-3]
                data = json.loads(text.strip())
                return data.get("missions", [])
            else:
                error_body = await resp.text()
                print(f"Gemini API returned status {resp.status}: {error_body}")
                raise RuntimeError("Failed to call Gemini API")


async def decompose_goal_openai(goal: str, api_key: str) -> list:
    """Query OpenAI API for structured goal decomposition."""
    url = "https://api.openai.com/v1/chat/completions"
    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {api_key}",
    }
    prompt = (
        f"Decompose the following human goal into a list of structured missions containing title, description, and priority (high/medium/low).\n"
        f"Goal: {goal}"
    )
    payload = {
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": prompt}],
        "response_format": {"type": "json_object"},
    }

    async with aiohttp.ClientSession() as session:
        async with session.post(url, headers=headers, json=payload) as resp:
            if resp.status == 200:
                result = await resp.json()
                text = result["choices"][0]["message"]["content"].strip()
                data = json.loads(text)
                return data.get("missions", [])
            else:
                error_body = await resp.text()
                print(f"OpenAI API returned status {resp.status}: {error_body}")
                raise RuntimeError("Failed to call OpenAI API")


async def message_handler(msg):
    """Handle NATS intent decomposition requests."""
    subject = msg.subject
    reply = msg.reply
    data = json.loads(msg.data.decode())
    goal_id = data.get("goal_id")
    goal = data.get("goal")

    print(f"Decomposing goal '{goal}' (ID: {goal_id})...")

    try:
        if GEMINI_API_KEY:
            missions = await decompose_goal_gemini(goal, GEMINI_API_KEY)
        elif OPENAI_API_KEY:
            missions = await decompose_goal_openai(goal, OPENAI_API_KEY)
        else:
            missions = await decompose_goal_mock(goal)

        response = {"missions": missions}
        response_bytes = json.dumps(response).encode()
        await nc.publish(reply, response_bytes)
        print(f"Responded to goal {goal_id} with {len(missions)} missions.")
    except Exception as e:
        print(f"Error decomposing goal {goal_id}: {e}")
        # Reply with empty list or error indicator
        err_response = {"missions": []}
        await nc.publish(reply, json.dumps(err_response).encode())


async def main():
    global nc
    nc = NATSClient()
    print(f"Connecting to NATS at {NATS_URL}...")
    await nc.connect(NATS_URL)
    print("Connected successfully.")

    # Subscribe to intent.decompose
    await nc.subscribe("intent.decompose", cb=message_handler)
    print("Subscribed to 'intent.decompose'. Agent is active.")

    # Keep agent running
    while True:
        await asyncio.sleep(1)


if __name__ == "__main__":
    asyncio.run(main())
