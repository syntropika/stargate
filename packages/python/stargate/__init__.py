"""Thin ASGI integration. Authentication and authorization execute in Rust."""

import functools
import inspect
import ipaddress
import json
from dataclasses import dataclass

from starlette.concurrency import run_in_threadpool
from starlette.exceptions import HTTPException
from starlette.requests import Request
from starlette.responses import JSONResponse

from ._native import Native


@dataclass(frozen=True)
class Turso:
    path: str
    connections: int = 16
    retry_limit: int = 32

    def config(self):
        return {
            "type": "turso",
            "path": self.path,
            "connections": self.connections,
            "retry_limit": self.retry_limit,
        }


@dataclass(frozen=True, repr=False)
class OIDC:
    issuer: str
    client_id: str
    client_secret: str
    name: str = "default"

    def config(self):
        return dict(vars(self))


@dataclass(frozen=True)
class Identity:
    subject: str
    user_id: str | None
    email: str | None
    auth_type: str
    scopes: list[str]
    claims: dict

    @property
    def id(self):
        return self.user_id or self.subject


class Auth:
    def __init__(self, *, base_url, storage, oidc=None, **options):
        configuration = {
            "base_url": base_url,
            "storage": storage.config(),
            "oidc": [
                p.config()
                for p in (oidc if isinstance(oidc, list) else [oidc] if oidc else [])
            ],
            **options,
        }
        self._native = Native(json.dumps(configuration))
        self._prefix = configuration.get("path_prefix", "/auth")
        self._body_limit = configuration.get("max_body_bytes", 65536)

    async def handle(self, request):
        return json.loads(
            await run_in_threadpool(self._native.handle, json.dumps(request))
        )

    async def authorize(self, identity, policy):
        if isinstance(identity, Identity):
            identity = vars(identity)
        return json.loads(
            await run_in_threadpool(
                self._native.authorize,
                json.dumps({"identity": identity, "policy": policy}),
            )
        )

    def mount(self, app):
        app.add_middleware(_Middleware, auth=self)
        return app

    def required(self):
        return self._protect({"type": "authenticated"})

    def require_scope(self, *scopes):
        return self._protect({"type": "scopes", "scopes": list(scopes)})

    def _protect(self, policy):
        def decorator(handler):
            signature = inspect.signature(handler, eval_str=True)
            if "user" not in signature.parameters:
                raise TypeError("Protected handlers must accept a 'user' parameter")
            if any(
                p.kind in (p.VAR_POSITIONAL, p.VAR_KEYWORD)
                for p in signature.parameters.values()
            ):
                raise TypeError("Protected handlers need an explicit signature")

            @functools.wraps(handler)
            async def wrapped(*args, **kwargs):
                request = kwargs.pop("_stargate_request", None)
                if request is None and args and isinstance(args[0], Request):
                    request = args[0]
                    if "request" not in signature.parameters:
                        args = args[1:]
                if request is None:
                    raise RuntimeError("Stargate protection requires a host request")
                identity = getattr(request.state, "stargate_identity", None)
                decision = await self.authorize(identity, policy)
                if not decision["allowed"]:
                    raise HTTPException(decision["status"], "Access denied")
                kwargs["user"] = Identity(**identity)
                if inspect.iscoroutinefunction(handler):
                    return await handler(*args, **kwargs)
                return await run_in_threadpool(handler, *args, **kwargs)

            parameters = [
                p for name, p in signature.parameters.items() if name != "user"
            ]
            parameters.append(
                inspect.Parameter(
                    "_stargate_request",
                    inspect.Parameter.KEYWORD_ONLY,
                    annotation=Request,
                )
            )
            wrapped.__signature__ = signature.replace(parameters=parameters)
            return wrapped

        return decorator


class _Middleware:
    def __init__(self, app, auth):
        self.app, self.auth = app, auth

    async def __call__(self, scope, receive, send):
        if scope["type"] != "http":
            return await self.app(scope, receive, send)
        # Prefer raw_path to avoid framework-specific percent decoding before the shared router.
        path = scope.get("raw_path", scope["path"].encode()).decode(
            "ascii", errors="replace"
        )
        owned = path == self.auth._prefix or path.startswith(self.auth._prefix + "/")
        body = bytearray()
        if owned:
            while True:
                message = await receive()
                if message["type"] == "http.disconnect":
                    return
                body.extend(message.get("body", b""))
                if len(body) > self.auth._body_limit:
                    return await JSONResponse({"error": "request too large"}, 413)(
                        scope, receive, send
                    )
                if not message.get("more_body", False):
                    break
        request = {
            "method": scope["method"],
            "path": path,
            "query": scope.get("query_string", b"").decode("ascii", errors="replace")
            or None,
            "headers": [
                [k.decode("ascii"), v.decode("latin1")] for k, v in scope["headers"]
            ],
            "body": list(body),
            "peer_ip": _peer_ip(scope),
        }
        outcome = await self.auth.handle(request)
        if outcome["type"] == "respond":
            response = outcome["response"]
            await send(
                {
                    "type": "http.response.start",
                    "status": response["status"],
                    "headers": [
                        (k.encode("ascii"), v.encode("latin1"))
                        for k, v in response["headers"]
                    ],
                }
            )
            await send({"type": "http.response.body", "body": bytes(response["body"])})
            return
        scope.setdefault("state", {})["stargate_identity"] = outcome["identity"]

        async def with_headers(message):
            if message["type"] == "http.response.start":
                message = {
                    **message,
                    "headers": list(message.get("headers", []))
                    + [
                        (k.encode(), v.encode()) for k, v in outcome["response_headers"]
                    ],
                }
            await send(message)

        await self.app(scope, receive, with_headers)


def _peer_ip(scope):
    try:
        return str(ipaddress.ip_address(scope["client"][0]))
    except (KeyError, TypeError, ValueError):
        return None
