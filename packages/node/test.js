const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const express = require('express');
const {createAuth,turso}=require('./index');
test('Express owns auth routes, preserves host bodies and enforces authentication', async () => {
  const dir=await fs.mkdtemp(path.join(os.tmpdir(),'stargate-express-'));
  const auth=await createAuth({baseUrl:'https://app.example.com',storage:turso(path.join(dir,'stargate.db'))});
  const app=express();app.use(auth.middleware());app.use(express.json());app.post('/echo',(req,res)=>res.json(req.body));app.get('/private',auth.required(),(req,res)=>res.json(req.stargateIdentity));
  const server=app.listen(0,'127.0.0.1');await new Promise(resolve=>server.once('listening',resolve));const base=`http://127.0.0.1:${server.address().port}`;
  try {
    assert.equal((await fetch(base+'/auth/')).status,200);
    assert.equal((await fetch(base+'/auth/assets/app.js')).status,200);
    assert.equal((await fetch(base+'/private')).status,401);
    assert.equal((await fetch(base+'/private',{headers:{authorization:'Bearer invalid'}})).status,401);
    assert.deepEqual(await (await fetch(base+'/echo',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({host:'body'})})).json(),{host:'body'});
    assert.equal((await fetch(base+'/auth/api/keys',{method:'POST',body:'x'.repeat(65537)})).status,413);
    assert.equal((await fetch(base+'/authentication')).status,404);
  } finally {await new Promise(resolve=>server.close(resolve));await fs.rm(dir,{recursive:true,force:true});}
});
