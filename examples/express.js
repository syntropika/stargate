const express = require('express');
const {createAuth,turso} = require('../packages/node');
(async () => {
  const auth = await createAuth({baseUrl:'https://localhost:3000',storage:turso('./stargate.db'),oidc:{issuer:'https://identity.example.com',clientId:'your-client-id',clientSecret:'your-client-secret'}});
  const app=express(); app.use(auth.middleware()); app.use(express.json());
  app.get('/private',auth.required(),(req,res)=>res.json({user:req.stargateIdentity.user_id}));
  app.listen(3000,'127.0.0.1');
})();
