import type {RequestHandler} from 'express';
export interface Identity {subject:string;user_id:string|null;email:string|null;auth_type:'session'|'api_key'|'bearer_jwt';scopes:string[];claims:Record<string,unknown>}
export interface Request {method:string;path:string;query?:string|null;headers?:[string,string][];body?:number[];peer_ip?:string|null}
export interface Response {status:number;headers:[string,string][];body:number[]}
export type Outcome = {type:'respond';response:Response}|{type:'continue';identity:Identity|null;response_headers:[string,string][]};
export type Policy = {type:'anonymous'}|{type:'authenticated'}|{type:'scopes';scopes:string[]};
export interface Config {
  baseUrl:string;storage:{type:'turso';path:string;connections?:number;retry_limit?:number};
  oidc?:{name?:string;issuer:string;clientId:string;clientSecret:string}|{name?:string;issuer:string;clientId:string;clientSecret:string}[];
  pathPrefix?:string;maxBodyBytes?:number;maxHeaderBytes?:number;trustedProxies?:string[];allowInsecureLoopback?:boolean;
  session?:{ttl_seconds?:number;scopes?:string[]};branding?:{app_name?:string;logo?:string|null;accent?:string};
}
export function turso(path:string,options?:{connections?:number;retry_limit?:number}):Config['storage'];
export function createAuth(config:Config):Promise<{handle(request:Request):Promise<Outcome>;authorize(identity:Identity|null,policy:Policy):Promise<{allowed:boolean;status:number}>;middleware():RequestHandler;handler():RequestHandler;required():RequestHandler;requireScope(...scopes:string[]):RequestHandler}>;
declare global {namespace Express {interface Request {stargateIdentity?:Identity|null}}}
