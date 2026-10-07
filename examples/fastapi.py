from fastapi import FastAPI
from stargate import OIDC, Auth, Turso

# Replace these explicitly with your host application's secret-management values.
app = FastAPI()
auth = Auth(
    base_url="https://localhost:3000",
    storage=Turso("./stargate.db"),
    session={"scopes": ["projects:write"]},
    oidc=OIDC(
        issuer="https://identity.example.com",
        client_id="your-client-id",
        client_secret="your-client-secret",
    ),
)
auth.mount(app)


@app.get("/private")
@auth.required()
async def private(user):
    return {"user": user.id}


@app.post("/projects")
@auth.require_scope("projects:write")
async def create_project(user):
    return {"owner": user.id}
