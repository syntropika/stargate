export const integrations = [
  {
    id: 'rust',
    label: 'Rust',
    framework: 'Rust / Axum',
    filename: 'server.rs',
    syntax: 'rust',
    command: 'cargo add syntropika-stargate --rename stargate',
    guide: 'rust--axum',
    code: `use axum::{routing::get, Router};
use stargate::{
    AuthConfig, LocalConfig, Policy, Stargate,
    StorageConfig, TursoConfig,
};

let mut config = AuthConfig::new(
    "https://app.example.com".parse()?,
    StorageConfig::Turso(TursoConfig::new("./stargate.db")),
);
config.local = Some(LocalConfig::default());
let auth = Stargate::new(config).await?;

let private = Stargate::require(
    Router::new().route("/private", get(|| async { "private" })),
    Policy::Authenticated,
);
let app = auth.mount(Router::new().merge(private));`,
  },
  {
    id: 'node',
    label: 'Node',
    framework: 'Node / Express',
    filename: 'server.ts',
    syntax: 'typescript',
    command: 'npm install @syntropika/stargate',
    guide: 'node--express',
    code: `import express from 'express';
import { createAuth, turso } from '@syntropika/stargate';

const app = express();
const auth = await createAuth({
  baseUrl: 'https://app.example.com',
  storage: turso('./stargate.db'),
  local: {},
});

app.use(auth.middleware());
app.get('/private', auth.required(), (req, res) => {
  res.json({ user: req.stargateIdentity.user_id });
});`,
  },
  {
    id: 'python',
    label: 'Python',
    framework: 'Python / FastAPI',
    filename: 'server.py',
    syntax: 'python',
    command: 'python -m pip install syntropika-stargate',
    guide: 'python--fastapi',
    code: `from fastapi import FastAPI
from stargate import Auth, Local, Turso

app = FastAPI()
auth = Auth(
    base_url="https://app.example.com",
    storage=Turso("./stargate.db"),
    local=Local(),
)
auth.mount(app)

@app.get("/private")
@auth.required()
async def private(user):
    return {"user": user.id}`,
  },
  {
    id: 'go',
    label: 'Go',
    framework: 'Go / net/http',
    filename: 'server.go',
    syntax: 'go',
    command: null,
    guide: 'go--nethttp',
    code: `import (
    "net/http"
    stargate "github.com/syntropika/stargate/packages/go"
)

auth, err := stargate.New(stargate.Config{
    BaseURL: "https://app.example.com",
    Storage: stargate.Turso("./stargate.db"),
    Local:   &stargate.LocalConfig{},
})
if err != nil { return err }
defer auth.Close()

mux := http.NewServeMux()
mux.Handle("/auth/", auth.Handler())
mux.Handle("/private", auth.Require(http.HandlerFunc(
    func(w http.ResponseWriter, r *http.Request) {
        w.Write([]byte("private"))
    },
)))`,
  },
] as const;
