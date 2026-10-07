"""Run the same WIT semantics, OIDC flow, keys and sessions through every native binding."""

import base64
import hashlib
import http.server
import json
import os
import subprocess
import tempfile
import threading
import time
import urllib.parse
import urllib.request
from pathlib import Path
from typing import ClassVar

import jwt
from cryptography.hazmat.primitives.asymmetric import rsa

ROOT = Path(__file__).resolve().parents[1]
KEY = rsa.generate_private_key(public_exponent=65537, key_size=2048)
PUBLIC = KEY.public_key().public_numbers()


def b64(value):
    return base64.urlsafe_b64encode(value).decode().rstrip("=")


class Idp(http.server.BaseHTTPRequestHandler):
    codes: ClassVar[dict[str, dict[str, str]]] = {}

    def log_message(self, *args):
        pass

    def send_json(self, value):
        body = json.dumps(value).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        parsed = urllib.parse.urlsplit(self.path)
        if parsed.path == "/.well-known/openid-configuration":
            self.send_json(
                {
                    "issuer": self.server.issuer,
                    "authorization_endpoint": self.server.issuer + "/authorize",
                    "token_endpoint": self.server.issuer + "/token",
                    "jwks_uri": self.server.issuer + "/jwks",
                    "response_types_supported": ["code"],
                    "subject_types_supported": ["public"],
                    "id_token_signing_alg_values_supported": ["RS256"],
                }
            )
        elif parsed.path == "/jwks":
            self.send_json(
                {
                    "keys": [
                        {
                            "kty": "RSA",
                            "use": "sig",
                            "kid": "test",
                            "alg": "RS256",
                            "n": b64(
                                PUBLIC.n.to_bytes(
                                    (PUBLIC.n.bit_length() + 7) // 8, "big"
                                )
                            ),
                            "e": b64(
                                PUBLIC.e.to_bytes(
                                    (PUBLIC.e.bit_length() + 7) // 8, "big"
                                )
                            ),
                        }
                    ]
                }
            )
        elif parsed.path == "/authorize":
            params = dict(urllib.parse.parse_qsl(parsed.query))
            assert params["code_challenge_method"] == "S256"
            code = os.urandom(24).hex()
            self.codes[code] = params
            self.send_json({"code": code, "state": params["state"]})
        else:
            self.send_error(404)

    def do_POST(self):
        params = dict(
            urllib.parse.parse_qsl(
                self.rfile.read(int(self.headers["content-length"])).decode()
            )
        )
        assert (
            self.headers["authorization"]
            == "Basic " + base64.b64encode(b"client:secret").decode()
        )
        flow = self.codes.pop(params["code"])
        assert (
            b64(hashlib.sha256(params["code_verifier"].encode()).digest())
            == flow["code_challenge"]
        )
        assert params["redirect_uri"] == flow["redirect_uri"]
        token = jwt.encode(
            {
                "iss": self.server.issuer,
                "sub": "test-user",
                "aud": "client",
                "iat": int(time.time()),
                "exp": int(time.time()) + 600,
                "nonce": flow["nonce"],
                "email": "person@example.com",
            },
            KEY,
            algorithm="RS256",
            headers={"kid": "test"},
        )
        self.send_json(
            {"id_token": token, "access_token": "mock-token", "token_type": "Bearer"}
        )


class Runner:
    def __init__(self, command, cwd=ROOT):
        self.process = subprocess.Popen(
            command, cwd=cwd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True
        )

    def call(self, operation, data):
        self.process.stdin.write(
            json.dumps({"operation": operation, "input": data}) + "\n"
        )
        self.process.stdin.flush()
        line = self.process.stdout.readline()
        assert line, "binding exited unexpectedly"
        out = json.loads(line)
        assert "error" not in out, "binding returned a transport error"
        return out

    def close(self):
        self.process.stdin.close()
        assert self.process.wait(timeout=20) == 0


def request(method, path, headers=None, body=None, query=None):
    return {
        "method": method,
        "path": path,
        "headers": headers or [],
        "body": list(json.dumps(body).encode()) if body is not None else [],
        "query": query,
        "peer_ip": None,
    }


def response(out, status):
    actual = out.get("response", {}).get("status", out.get("type"))
    assert out["type"] == "respond" and out["response"]["status"] == status, (
        f"expected HTTP {status}, received {actual}"
    )
    return out["response"]


def payload(r):
    return json.loads(bytes(r["body"]))


def header(r, name):
    return next(v for k, v in r["headers"] if k == name)


def login(runner):
    start = response(
        runner.call(
            "handle", request("GET", "/auth/login", query="return_to=/private")
        ),
        303,
    )
    browser = header(start, "set-cookie").split(";")[0]
    with urllib.request.urlopen(header(start, "location")) as r:
        callback = json.load(r)
    query = urllib.parse.urlencode(callback)
    response(runner.call("handle", request("GET", "/auth/callback", query=query)), 401)
    completed = response(
        runner.call(
            "handle",
            request("GET", "/auth/callback", [["cookie", browser]], query=query),
        ),
        303,
    )
    session = next(
        v.split(";")[0]
        for k, v in completed["headers"]
        if k == "set-cookie" and v.startswith("__Host-stargate-session=")
    )
    response(
        runner.call(
            "handle",
            request("GET", "/auth/callback", [["cookie", browser]], query=query),
        ),
        401,
    )
    me = payload(
        response(
            runner.call(
                "handle", request("GET", "/auth/api/me", [["cookie", session]])
            ),
            200,
        )
    )
    return session, me


def suite(runner, issuer, path):
    ready = runner.call(
        "create",
        {
            "base_url": "https://app.example.com",
            "storage": {"type": "turso", "path": str(path)},
            "allow_insecure_loopback": True,
            "oidc": [
                {
                    "name": "test",
                    "issuer": issuer,
                    "client_id": "client",
                    "client_secret": "secret",
                }
            ],
            "session": {
                "ttl_seconds": 60,
                "scopes": ["projects:read", "projects:write"],
            },
        },
    )
    assert ready["ready"]
    for case in json.loads((ROOT / "conformance/cases.json").read_text()):
        data = case["input"]
        out = runner.call(case["operation"], data)
        expected = case["expected"]
        for name, value in expected.items():
            actual = (
                out["response"]["status"]
                if name == "status" and out.get("type") == "respond"
                else out.get(name)
            )
            assert actual == value, case["name"]
    session, me = login(runner)
    cookie = [["cookie", session]]
    csrf = cookie + [
        ["origin", "https://app.example.com"],
        ["x-stargate-csrf", me["csrf_token"]],
        ["content-type", "application/json"],
    ]
    user = me["identity"]
    assert user["auth_type"] == "session"
    assert runner.call("handle", request("GET", "/private", cookie))["identity"] == user
    assert runner.call(
        "authorize",
        {"identity": user, "policy": {"type": "scopes", "scopes": ["projects:write"]}},
    )["allowed"]
    assert (
        runner.call(
            "authorize",
            {"identity": user, "policy": {"type": "scopes", "scopes": ["admin"]}},
        )["status"]
        == 403
    )
    response(runner.call("handle", request("POST", "/auth/logout", cookie)), 403)
    created = payload(
        response(
            runner.call(
                "handle",
                request(
                    "POST",
                    "/auth/api/keys",
                    csrf,
                    {"name": "conformance", "scopes": ["projects:read"]},
                ),
            ),
            201,
        )
    )
    key = created["secret"]
    assert key.startswith("ak_live_")
    bearer = [["authorization", "Bearer " + key]]
    identity = runner.call("handle", request("GET", "/private", bearer))["identity"]
    assert identity["auth_type"] == "api_key"
    assert identity["user_id"] == user["user_id"]
    assert (
        runner.call(
            "authorize",
            {
                "identity": identity,
                "policy": {"type": "scopes", "scopes": ["projects:write"]},
            },
        )["status"]
        == 403
    )
    response(runner.call("handle", request("GET", "/auth/api/keys", bearer)), 403)
    listed = payload(
        response(runner.call("handle", request("GET", "/auth/api/keys", cookie)), 200)
    )
    assert "secret_hash" not in listed[0] and key not in json.dumps(listed)
    response(
        runner.call(
            "handle", request("DELETE", "/auth/api/keys/" + created["key"]["id"], csrf)
        ),
        204,
    )
    response(runner.call("handle", request("GET", "/private", bearer)), 401)
    expired = payload(
        response(
            runner.call(
                "handle",
                request(
                    "POST",
                    "/auth/api/keys",
                    csrf,
                    {"name": "short-lived", "expires_at": int(time.time()) + 3},
                ),
            ),
            201,
        )
    )
    time.sleep(max(0, expired["key"]["expires_at"] - time.time()) + 0.1)
    response(
        runner.call(
            "handle",
            request(
                "GET", "/private", [["authorization", "Bearer " + expired["secret"]]]
            ),
        ),
        401,
    )
    response(runner.call("handle", request("DELETE", "/auth/api/sessions", csrf)), 204)
    response(runner.call("handle", request("GET", "/private", cookie)), 401)
    runner.call(
        "create",
        {
            "base_url": "https://app.example.com",
            "storage": {"type": "turso", "path": str(path.parent / "expiration.db")},
            "allow_insecure_loopback": True,
            "oidc": [
                {
                    "name": "test",
                    "issuer": issuer,
                    "client_id": "client",
                    "client_secret": "secret",
                }
            ],
            "session": {"ttl_seconds": 2},
        },
    )
    short_session, _ = login(runner)
    assert (
        runner.call("handle", request("GET", "/private", [["cookie", short_session]]))[
            "type"
        ]
        == "continue"
    )
    time.sleep(2.1)
    response(
        runner.call("handle", request("GET", "/private", [["cookie", short_session]])),
        401,
    )
    for file in path.parent.glob("*"):
        if file.is_file():
            data = file.read_bytes()
            assert (
                key.encode() not in data
                and session.split("=", 1)[1].encode() not in data
            )


def main():
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Idp)
    server.issuer = f"http://127.0.0.1:{server.server_port}"
    threading.Thread(target=server.serve_forever, daemon=True).start()
    commands = {
        "rust": ([str(ROOT / "target/debug/examples/conformance")], ROOT),
        "python": (
            [
                os.environ.get("STARGATE_TEST_PYTHON", "python3"),
                str(ROOT / "conformance/python.py"),
            ],
            ROOT,
        ),
        "node": (["node", str(ROOT / "conformance/node.js")], ROOT),
        "go": ([str(ROOT / "target/debug/go-conformance")], ROOT),
    }
    try:
        for name, (command, cwd) in commands.items():
            with tempfile.TemporaryDirectory(
                prefix="stargate-" + name + "-"
            ) as directory:
                runner = Runner(command, cwd)
                try:
                    suite(runner, server.issuer, Path(directory) / "stargate.db")
                finally:
                    runner.close()
                print(
                    f"{name}: shared contract, OIDC, CSRF, sessions, keys, scopes, expiration and revocation passed",
                    flush=True,
                )
    finally:
        server.shutdown()
        server.server_close()


if __name__ == "__main__":
    main()
