'use strict';
const {Native} = require('./native');
const turso = (path, options = {}) => ({type:'turso', path, ...options});
async function createAuth(configuration) {
  const {baseUrl, pathPrefix, maxBodyBytes, maxHeaderBytes, trustedProxies, allowInsecureLoopback, oidc, ...rest} = configuration;
  const config = {...rest, base_url: baseUrl, ...(pathPrefix === undefined ? {} : {path_prefix:pathPrefix}), ...(maxBodyBytes === undefined ? {} : {max_body_bytes:maxBodyBytes}), ...(maxHeaderBytes === undefined ? {} : {max_header_bytes:maxHeaderBytes}), ...(trustedProxies === undefined ? {} : {trusted_proxies:trustedProxies}), ...(allowInsecureLoopback === undefined ? {} : {allow_insecure_loopback:allowInsecureLoopback}), oidc: (oidc ? (Array.isArray(oidc) ? oidc : [oidc]) : []).map(({clientId,clientSecret,name='default',issuer}) => ({name,issuer,client_id:clientId,client_secret:clientSecret}))};
  const native = await Native.create(JSON.stringify(config));
  const handle = async request => JSON.parse(await native.handle(JSON.stringify(request)));
  const authorize = async (identity, policy) => JSON.parse(await native.authorize(JSON.stringify({identity:identity || null,policy})));
  const middleware = () => async (req,res,next) => {
    try {
      const raw = req.originalUrl || req.url, separator = raw.indexOf('?');
      const path = separator < 0 ? raw : raw.slice(0,separator), query = separator < 0 ? null : raw.slice(separator+1);
      const prefix = config.path_prefix || '/auth';
      const owned = path === prefix || path.startsWith(prefix+'/');
      let body = Buffer.alloc(0);
      if (owned) {
        if (req.readableEnded || req.body !== undefined) throw new Error('Mount Stargate before body parsers');
        const chunks = []; let size = 0;
        for await (const chunk of req) { size += chunk.length; if (size > (config.max_body_bytes || 65536)) {res.statusCode=413;res.end();return;} chunks.push(chunk); }
        body = Buffer.concat(chunks);
      }
      const headers = []; for (let i=0;i<req.rawHeaders.length;i+=2) headers.push([req.rawHeaders[i],req.rawHeaders[i+1]]);
      const peer = req.socket.remoteAddress;
      const outcome = await handle({method:req.method,path,query,headers,body:[...body],peer_ip:peer || null});
      if (outcome.type === 'continue') { req.stargateIdentity = outcome.identity; for (const [key,value] of outcome.response_headers) res.append(key,value); return next(); }
      res.statusCode = outcome.response.status;
      const grouped = new Map(); for (const [key,value] of outcome.response.headers) { const list=grouped.get(key)||[];list.push(value);grouped.set(key,list); }
      for (const [key,values] of grouped) res.setHeader(key,values.length === 1 ? values[0] : values);
      res.end(Buffer.from(outcome.response.body));
    } catch(error) {next(error);}
  };
  const protect = policy => async (req,res,next) => { try { const decision = await authorize(req.stargateIdentity,policy); if (!decision.allowed) return res.status(decision.status).json({error:'Access denied'}); next(); } catch(error) {next(error);} };
  return {handle,authorize,middleware,handler:middleware,required:()=>protect({type:'authenticated'}),requireScope:(...scopes)=>protect({type:'scopes',scopes})};
}
module.exports = {createAuth,turso};
