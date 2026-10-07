import json
import sys
import threading
import urllib.request
from pathlib import Path

import pytest
from fastapi import FastAPI, Request
from stargate import OIDC, Auth, Turso
from starlette.testclient import TestClient

sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
import http.server

from conformance.run import Idp


@pytest.fixture
def provider():
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Idp)
    server.issuer = f"http://127.0.0.1:{server.server_port}"
    threading.Thread(target=server.serve_forever, daemon=True).start()
    yield server.issuer
    server.shutdown()
    server.server_close()


def make_app(path, provider=None):
    app = FastAPI()
    auth = Auth(
        base_url="https://app.example.com",
        storage=Turso(str(path)),
        oidc=OIDC(provider, "client", "secret") if provider else None,
        allow_insecure_loopback=True,
        session={"scopes": ["projects:read", "projects:write"]},
    )
    auth.mount(app)

    @app.get("/private")
    @auth.required()
    async def private(user):
        return {"user": user.id, "type": user.auth_type}

    @app.post("/projects/{project_id}")
    @auth.require_scope("projects:write")
    def project(project_id: str, user):
        return {"project": project_id, "owner": user.id}

    @app.post("/echo")
    async def echo(request: Request):
        return {"body": (await request.body()).decode()}

    return app


def test_real_fastapi_oidc_session_key_and_csrf(tmp_path, provider):
    app = make_app(tmp_path / "stargate.db", provider)
    with TestClient(
        app, base_url="https://app.example.com", follow_redirects=False
    ) as client:
        assert client.get("/private").status_code == 401
        assert client.post("/echo", content="host-body").json()["body"] == "host-body"
        start = client.get("/auth/login?return_to=/private")
        with urllib.request.urlopen(start.headers["location"]) as r:
            flow = json.load(r)
        callback = client.get("/auth/callback", params=flow)
        assert callback.status_code == 303
        private = client.get("/private")
        assert private.status_code == 200 and private.json()["type"] == "session"
        assert client.post("/projects/one").status_code == 200
        me = client.get("/auth/api/me").json()
        assert (
            client.post("/auth/api/keys", json={"name": "blocked"}).status_code == 403
        )
        headers = {
            "origin": "https://app.example.com",
            "x-stargate-csrf": me["csrf_token"],
        }
        key = client.post(
            "/auth/api/keys",
            json={"name": "integration", "scopes": ["projects:read"]},
            headers=headers,
        )
        assert key.status_code == 201
        bearer = {"authorization": "Bearer " + key.json()["secret"]}
        assert client.get("/private", headers=bearer).json()["type"] == "api_key"
        assert client.post("/projects/one", headers=bearer).status_code == 403
        assert (
            client.delete(
                "/auth/api/keys/" + key.json()["key"]["id"], headers=headers
            ).status_code
            == 204
        )
        assert client.get("/private", headers=bearer).status_code == 401
        assert client.post("/auth/logout", headers=headers).status_code == 204
        assert client.get("/private").status_code == 401


def test_owned_routes_limits_and_assets(tmp_path):
    app = make_app(tmp_path / "stargate.db")
    with TestClient(app, base_url="https://app.example.com") as client:
        assert client.get("/auth/").status_code == 200
        assert client.get("/auth/assets/app.js").status_code == 200
        assert (
            client.get("/auth/assets/app.css")
            .headers["content-type"]
            .startswith("text/css")
        )
        assert client.post("/auth/api/keys", content=b"x" * 65537).status_code == 413
        assert client.get("/authentication").status_code == 404


def test_starlette_protection(tmp_path):
    from starlette.applications import Starlette
    from starlette.responses import JSONResponse
    from starlette.routing import Route

    auth = Auth(
        base_url="https://app.example.com", storage=Turso(str(tmp_path / "stargate.db"))
    )

    @auth.required()
    async def private(request, user):
        return JSONResponse({"user": user.id})

    app = Starlette(routes=[Route("/private", private)])
    auth.mount(app)
    with TestClient(app, base_url="https://app.example.com") as client:
        assert client.get("/private").status_code == 401
        assert client.get("/auth/").status_code == 200
