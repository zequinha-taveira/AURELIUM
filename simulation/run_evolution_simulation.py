import asyncio
import json
import os
import re
from nats.aio.client import Client as NATSClient

NATS_URL = os.getenv("NATS_URL", "nats://localhost:4222")


async def main():
    nc = NATSClient()
    print(f"Connecting to NATS at {NATS_URL}...")
    await nc.connect(NATS_URL)
    print("Connected successfully.")

    # Queue to store intercepted variant registrations
    registered_variants = asyncio.Queue()

    async def register_handler(msg):
        yaml_data = msg.data.decode("utf-8")
        # Simple regex to extract the id from the registered YAML
        match = re.search(r'^id:\s*"(capability\.db_query@v[^"]+)"', yaml_data, re.MULTILINE)
        if match:
            variant_id = match.group(1)
            print(f"[NATS Intercept] Intercepted registration of variant: {variant_id}")
            await registered_variants.put(variant_id)

    await nc.subscribe("genome.register", cb=register_handler)
    print("Subscribed to 'genome.register' to capture variants.")

    # 1. Register the baseline capability
    genomes_dir = os.path.join("infrastructure", "genomes")
    baseline_file = os.path.join(genomes_dir, "capability.db_query.yaml")
    if not os.path.exists(baseline_file):
        print(f"Error: {baseline_file} not found!")
        await nc.close()
        exit(1)

    with open(baseline_file, "r", encoding="utf-8") as f:
        content = f.read()

    print("Registering baseline capability: capability.db_query")
    await nc.publish("genome.register", content.encode("utf-8"))
    await asyncio.sleep(1)

    # 2. Trigger first mutation
    print("\n--- Triggering Mutation 1 ---")
    req_mutate = json.dumps({"capability_id": "capability.db_query"}).encode("utf-8")
    await nc.publish("evolution.mutate", req_mutate)

    try:
        # Wait for the variant registration event
        variant_1 = await asyncio.wait_for(registered_variants.get(), timeout=5)
        print(f"Successfully captured Mutation 1 Variant ID: {variant_1}")
    except asyncio.TimeoutError:
        print("Error: Timeout waiting for mutation variant registration!")
        await nc.close()
        exit(1)

    # 3. Report telemetry for both baseline and variant 1
    # Variant 1 is simulated to be significantly better (lower latency, higher success)
    print("\nReporting telemetry for baseline (latency=50ms, success=95%)")
    await nc.publish(
        "evolution.telemetry.report",
        json.dumps({
            "variant_id": "capability.db_query",
            "latency_ms": 50.0,
            "error_rate": 0.0,
            "success_rate": 0.95
        }).encode("utf-8")
    )

    print(f"Reporting telemetry for variant {variant_1} (latency=10ms, success=100%)")
    await nc.publish(
        "evolution.telemetry.report",
        json.dumps({
            "variant_id": variant_1,
            "latency_ms": 10.0,
            "error_rate": 0.0,
            "success_rate": 1.0
        }).encode("utf-8")
    )
    await asyncio.sleep(1)

    # 4. Trigger evaluation for Mutation 1
    print("\nEvaluating Mutation 1...")
    req_eval = json.dumps({"capability_id": "capability.db_query"}).encode("utf-8")
    try:
        msg = await nc.request("evolution.evaluate", req_eval, timeout=5)
        response = json.loads(msg.data.decode("utf-8"))
        print(f"Evaluation Response: {response}")
        assert response.get("status") == "promoted", "Mutation 1 should have been promoted!"
        assert response.get("winner") == variant_1, "Mutation 1 should be the winner!"
        print("Success: Mutation 1 promoted to baseline!")
    except Exception as e:
        print(f"Error during evaluation: {e}")
        await nc.close()
        exit(1)

    # 5. Trigger second mutation
    print("\n--- Triggering Mutation 2 ---")
    await nc.publish("evolution.mutate", req_mutate)

    try:
        variant_2 = await asyncio.wait_for(registered_variants.get(), timeout=5)
        print(f"Successfully captured Mutation 2 Variant ID: {variant_2}")
    except asyncio.TimeoutError:
        print("Error: Timeout waiting for mutation 2 variant registration!")
        await nc.close()
        exit(1)

    # 6. Report poor telemetry for variant 2
    # Variant 2 is simulated to be much worse (high latency, high error rate)
    print(f"\nReporting telemetry for variant {variant_2} (latency=300ms, success=50%, error=50%)")
    await nc.publish(
        "evolution.telemetry.report",
        json.dumps({
            "variant_id": variant_2,
            "latency_ms": 300.0,
            "error_rate": 0.5,
            "success_rate": 0.5
        }).encode("utf-8")
    )
    await asyncio.sleep(1)

    # 7. Trigger evaluation for Mutation 2
    print("\nEvaluating Mutation 2...")
    try:
        msg = await nc.request("evolution.evaluate", req_eval, timeout=5)
        response = json.loads(msg.data.decode("utf-8"))
        print(f"Evaluation Response: {response}")
        assert response.get("status") == "extinguished", "Mutation 2 should have been extinguished!"
        print("Success: Mutation 2 extinguished due to poor performance!")
    except Exception as e:
        print(f"Error during evaluation: {e}")
        await nc.close()
        exit(1)

    await nc.close()
    print("\nAll Evolution Engine simulation tests passed successfully!")


if __name__ == "__main__":
    asyncio.run(main())
