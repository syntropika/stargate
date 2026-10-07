const readline = require('node:readline');
const {createAuth} = require('../packages/node');
(async () => {
  let auth;
  for await (const line of readline.createInterface({input:process.stdin})) {
    const {operation,input}=JSON.parse(line); let output;
    try {
      if (operation==='create') {
        const {base_url,path_prefix,allow_insecure_loopback,max_body_bytes,max_header_bytes,trusted_proxies,oidc,...rest}=input;
        auth=await createAuth({...rest,baseUrl:base_url,pathPrefix:path_prefix,allowInsecureLoopback:allow_insecure_loopback,maxBodyBytes:max_body_bytes,maxHeaderBytes:max_header_bytes,trustedProxies:trusted_proxies,oidc:oidc.map(({client_id,client_secret,...p})=>({...p,clientId:client_id,clientSecret:client_secret}))});output={ready:true};
      } else if (operation==='handle') output=await auth.handle(input);
      else output=await auth.authorize(input.identity,input.policy);
    } catch {output={error:'native call failed'};}
    process.stdout.write(JSON.stringify(output)+'\n');
  }
})();
