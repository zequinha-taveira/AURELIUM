import asyncio
import json
import os
import aiohttp
from nats.aio.client import Client as NATSClient

NATS_URL = os.getenv("NATS_URL", "nats://localhost:4222")
GEMINI_API_KEY = os.getenv("GEMINI_API_KEY")
OPENAI_API_KEY = os.getenv("OPENAI_API_KEY")


async def generate_backend_mock(title: str, description: str) -> str:
    """Fallback generator for mock backend code."""
    return f"""// Backend implementation for task: {title}
// Description: {description}

use axum::{{routing::get, Json, Router}};
use serde::Serialize;

#[derive(Serialize)]
struct StatusResponse {{
    status: String,
    task: String,
}}

pub fn router() -> Router {{
    Router::new().route("/api/task/status", get(get_status))
}}

async fn get_status() -> Json<StatusResponse> {{
    Json(StatusResponse {{
        status: "operational".to_string(),
        task: "{title}".to_string(),
    }})
}}
"""


async def query_llm_backend(title: str, description: str) -> str:
    """Queries OpenAI or Gemini to generate backend code."""
    prompt = (
        f"You are the AURELIUM Backend Agent. Generate clean, operational Rust/Axum or Node.js/TS code "
        f"for this backend task: '{title}'. Description: '{description}'. "
        f"Respond ONLY with raw source code, no explanations or markdown blocks."
    )

    if GEMINI_API_KEY:
        url = f"https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key={GEMINI_API_KEY}"
        payload = {"contents": [{"parts": [{"text": prompt}]}]}
        async with aiohttp.ClientSession() as session:
            async with session.post(url, json=payload) as resp:
                if resp.status == 200:
                    res = await resp.json()
                    text = res["candidates"][0]["content"]["parts"][0]["text"].strip()
                    if text.startswith("```"):
                        text = "\n".join(text.split("\n")[1:-1])
                    return text
    elif OPENAI_API_KEY:
        url = "https://api.openai.com/v1/chat/completions"
        headers = {"Authorization": f"Bearer {OPENAI_API_KEY}"}
        payload = {
            "model": "gpt-4o-mini",
            "messages": [{"role": "user", "content": prompt}],
        }
        async with aiohttp.ClientSession() as session:
            async with session.post(url, headers=headers, json=payload) as resp:
                if resp.status == 200:
                    res = await resp.json()
                    return res["choices"][0]["message"]["content"].strip()
                    
    return await generate_backend_mock(title, description)


async def task_handler(msg):
    data = json.loads(msg.data.decode())
    task_id = data.get("task_id")
    title = data.get("title")
    description = data.get("description")

    print(f"Backend Agent starting work on Task: {task_id} - '{title}'")

    try:
        # 1. Generate code
        generated_code = await query_llm_backend(title, description)
        print(f"Generated code for Task {task_id}. Requesting security audit...")

        # 2. Collaboration Protocol: request security review via NATS RPC
        review_req = {
            "task_id": task_id,
            "title": title,
            "code": generated_code,
        }
        
        # Request-Reply pattern to security-agent with 15s timeout
        review_reply = await nc.request(
            "agent.security.review",
            json.dumps(review_req).encode(),
            timeout=15
        )
        
        review_data = json.loads(review_reply.data.decode())
        approved = review_data.get("approved", False)
        report = review_data.get("report", "No security report provided.")

        print(f"Security audit for Task {task_id} result: Approved={approved}")

        # 3. Report resolution to Coordination Engine
        resolution = {
            "task_id": task_id,
            "status": "completed" if approved else "failed",
            "output": f"CODE:\n{generated_code}\n\nSECURITY REPORT:\n{report}",
            "security_approval": approved,
        }
        await nc.publish("agent.task.resolved", json.dumps(resolution).encode())
        print(f"Backend Agent finalized Task {task_id}.")

    except Exception as e:
        print(f"Backend Agent error on Task {task_id}: {e}")
        err_resolution = {
            "task_id": task_id,
            "status": "failed",
            "output": f"Backend Agent failed: {e}",
            "security_approval": False,
        }
        await nc.publish("agent.task.resolved", json.dumps(err_resolution).encode())


async def main():
    global nc
    nc = NATSClient()
    print(f"Backend Agent connecting to NATS at {NATS_URL}...")
    await nc.connect(NATS_URL)
    print("Connected.")

    await nc.subscribe("agent.backend.assign", cb=task_handler)
    print("Subscribed to 'agent.backend.assign'. Standing by...")

    while True:
        await asyncio.sleep(1)


if __name__ == "__main__":
    asyncio.run(main())
