import asyncio
import json
import os
from nats.aio.client import Client as NATSClient

NATS_URL = os.getenv("NATS_URL", "nats://localhost:4222")


async def main():
    nc = NATSClient()
    print(f"Connecting to NATS at {NATS_URL}...")
    await nc.connect(NATS_URL)
    print("Connected successfully.")

    # 1. Read YAML genome files
    genomes_dir = os.path.join("infrastructure", "genomes")
    files = ["capability.db_query.yaml", "capability.auth_validate.yaml", "capability.payment_process.yaml"]

    print("Publishing genome seed files...")
    for filename in files:
        filepath = os.path.join(genomes_dir, filename)
        if not os.path.exists(filepath):
            print(f"Error: File {filepath} not found!")
            continue

        with open(filepath, "r", encoding="utf-8") as f:
            content = f.read()

        print(f"Registering genome: {filename}")
        await nc.publish("genome.register", content.encode("utf-8"))

    # Give the engine a second to process and write to Postgres and Neo4j
    await asyncio.sleep(2)

    # 2. Test genome.get_dependencies RPC query
    test_id = "capability.payment_process"
    print(f"\nQuerying recursive dependencies for '{test_id}'...")
    req_payload = json.dumps({"capability_id": test_id}).encode("utf-8")
    
    try:
        msg = await nc.request("genome.get_dependencies", req_payload, timeout=5)
        response = json.loads(msg.data.decode("utf-8"))
        dependencies = response.get("dependencies", [])
        print(f"Resolved recursive dependencies: {dependencies}")
        
        # Verify that dependencies contain both auth_validate and db_query
        assert "capability.auth_validate" in dependencies, "Missing auth_validate dependency!"
        assert "capability.db_query" in dependencies, "Missing db_query dependency!"
        print("Success: Dependencies correctly resolved via Neo4j!")
    except Exception as e:
        print(f"Error querying dependencies: {e}")
        exit(1)

    # 3. Test genome.validate RPC query for success
    print(f"\nValidating capability '{test_id}'...")
    try:
        msg = await nc.request("genome.validate", req_payload, timeout=5)
        response = json.loads(msg.data.decode("utf-8"))
        valid = response.get("valid", False)
        missing = response.get("missing_dependencies", [])
        print(f"Validation status: {valid}, Missing: {missing}")
        assert valid is True, "Validation should have succeeded!"
        assert len(missing) == 0, "There should be no missing dependencies!"
        print("Success: Validation succeeded as expected!")
    except Exception as e:
        print(f"Error validating capability: {e}")
        exit(1)

    # 4. Test genome.validate RPC query for missing dependency
    missing_test_id = "capability.payment_process_invalid"
    # Register an invalid capability with a non-existent dependency
    invalid_yaml = """
id: "capability.payment_process_invalid"
name: "Invalid Payment Processing"
version: "1.0.0"
dependencies:
  - "capability.non_existent_dependency"
"""
    print(f"\nRegistering temporary invalid capability '{missing_test_id}'...")
    await nc.publish("genome.register", invalid_yaml.encode("utf-8"))
    await asyncio.sleep(1)

    print(f"Validating capability '{missing_test_id}'...")
    req_payload_invalid = json.dumps({"capability_id": missing_test_id}).encode("utf-8")
    try:
        msg = await nc.request("genome.validate", req_payload_invalid, timeout=5)
        response = json.loads(msg.data.decode("utf-8"))
        valid = response.get("valid", False)
        missing = response.get("missing_dependencies", [])
        print(f"Validation status: {valid}, Missing: {missing}")
        assert valid is False, "Validation should have failed!"
        assert "capability.non_existent_dependency" in missing, "Missing dependency list incorrect!"
        print("Success: Validation correctly reported missing dependency!")
    except Exception as e:
        print(f"Error validating invalid capability: {e}")
        exit(1)

    await nc.close()
    print("\nAll Semantic Genome integration tests passed successfully!")


if __name__ == "__main__":
    asyncio.run(main())
