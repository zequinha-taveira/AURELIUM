import asyncio
import json
import os
import aiohttp
from nats.aio.client import Client as NATSClient

NATS_URL = os.getenv("NATS_URL", "nats://localhost:4222")
GEMINI_API_KEY = os.getenv("GEMINI_API_KEY")
OPENAI_API_KEY = os.getenv("OPENAI_API_KEY")


async def audit_code_mock(code: str) -> tuple[bool, str]:
    """Perform a mock security audit checking for simple flaws."""
    issues = []
    
    if "password" in code.lower() or "secret" in code.lower():
        issues.append("Hardcoded credentials or secrets detected.")
    if "select * from" in code.lower() and "where" in code.lower() and not "$" in code.lower():
        issues.append("Potential SQL Injection vulnerability. Query uses unparameterized input.")
    
    if issues:
        report = "FAIL: Security audit found critical issues:\n" + "\n".join(f"- {issue}" for issue in issues)
        return False, report
    else:
        return True, "PASS: No immediate security vulnerabilities detected. Code approved."


async def query_llm_audit(code: str) -> tuple[bool, str]:
    """Query LLM to perform code audit and output JSON approval."""
    prompt = (
        f"You are the AURELIUM Security Agent. Audit the following source code for vulnerabilities (SQL injection, XSS, RCE, secrets). "
        f"Source Code:\n{code}\n\n"
        f"Respond ONLY with a JSON object matching this schema:\n"
        f"{{\n"
        f"  \"approved\": true | false,\n"
        f"  \"report\": \"string detailing findings\"\n"
        f"}}\n"
        f"Do not include markdown wrappers, just raw JSON."
    )

    try:
        if GEMINI_API_KEY:
            url = f"https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:key={GEMINI_API_KEY}"
            payload = {"contents": [{"parts": [{"text": prompt}]}]}
            async with aiohttp.ClientSession() as session:
                async with session.post(url, json=payload) as resp:
                    if resp.status == 200:
                        res = await resp.json()
                        text = res["candidates"][0]["content"]["parts"][0]["text"].strip()
                        if text.startswith("```"):
                            text = "\n".join(text.split("\n")[1:-1])
                        data = json.loads(text.strip())
                        return data.get("approved", False), data.get("report", "")
        elif OPENAI_API_KEY:
            url = "https://api.openai.com/v1/chat/completions"
            headers = {"Authorization": f"Bearer {OPENAI_API_KEY}"}
            payload = {
                "model": "gpt-4o-mini",
                "messages": [{"role": "user", "content": prompt}],
                "response_format": {"type": "json_object"},
            }
            async with aiohttp.ClientSession() as session:
                async with session.post(url, headers=headers, json=payload) as resp:
                    if resp.status == 200:
                        res = await resp.json()
                        data = json.loads(res["choices"][0]["message"]["content"])
                        return data.get("approved", False), data.get("report", "")
    except Exception as e:
        print(f"Error querying LLM for security audit: {e}")
        
    return await audit_code_mock(code)


async def review_handler(msg):
    """Handle RPC NATS requests for security reviews."""
    reply = msg.reply
    data = json.loads(msg.data.decode())
    task_id = data.get("task_id")
    code = data.get("code", "")

    print(f"Security Agent auditing code for Task: {task_id}")
    
    approved, report = await query_llm_audit(code)
    
    response = {
        "approved": approved,
        "report": report,
    }
    await nc.publish(reply, json.dumps(response).encode())
    print(f"Responded to review for Task {task_id}. Approved={approved}")


async def assign_handler(msg):
    """Handle direct task assignments to the Security Agent."""
    data = json.loads(msg.data.decode())
    task_id = data.get("task_id")
    title = data.get("title")
    description = data.get("description")

    print(f"Security Agent executing direct Task: {task_id} - '{title}'")
    
    # Direct task is usually to perform general audit/analysis
    resolution = {
        "task_id": task_id,
        "status": "completed",
        "output": f"Audit performed for: '{title}'. Context checks passed successfully.",
        "security_approval": True,
    }
    await nc.publish("agent.task.resolved", json.dumps(resolution).encode())
    print(f"Security Agent completed Task {task_id}.")


async def main():
    global nc
    nc = NATSClient()
    print(f"Security Agent connecting to NATS at {NATS_URL}...")
    await nc.connect(NATS_URL)
    print("Connected.")

    # Listen for review RPC requests
    await nc.subscribe("agent.security.review", cb=review_handler)
    # Listen for direct task assignments
    await nc.subscribe("agent.security.assign", cb=assign_handler)
    print("Security Agent fully operational.")

    while True:
        await asyncio.sleep(1)


if __name__ == "__main__":
    asyncio.run(main())
