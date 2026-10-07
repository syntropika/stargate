import asyncio
import json
import sys

from stargate import OIDC, Auth, Turso


async def main():
    auth = None
    for line in sys.stdin:
        command = json.loads(line)
        operation, data = command["operation"], command["input"]
        try:
            if operation == "create":
                data = dict(data)
                storage = data.pop("storage")
                data["storage"] = Turso(
                    storage["path"],
                    storage.get("connections", 16),
                    storage.get("retry_limit", 32),
                )
                data["oidc"] = [OIDC(**p) for p in data.get("oidc", [])]
                auth = await asyncio.to_thread(Auth, **data)
                output = {"ready": True}
            elif operation == "handle":
                output = await auth.handle(data)
            else:
                output = await auth.authorize(data.get("identity"), data["policy"])
        except (ValueError, TypeError, RuntimeError):
            output = {"error": "native call failed"}
        print(json.dumps(output), flush=True)


asyncio.run(main())
