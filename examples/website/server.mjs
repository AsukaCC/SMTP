import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {BunMailClient, BunMailError} from '../client.mjs';

const from=process.env.MAIL_FROM,to=process.env.MAIL_TO;
const client=process.env.BUNMAIL_API_URL&&process.env.BUNMAIL_API_KEY&&from&&to
  ?new BunMailClient({baseUrl:process.env.BUNMAIL_API_URL,apiKey:process.env.BUNMAIL_API_KEY}):null;
const assets=new Map(await Promise.all([
  ['/', './index.html', 'text/html; charset=utf-8'],
  ['/app.js', './app.js', 'text/javascript; charset=utf-8'],
  ['/app.css', './app.css', 'text/css; charset=utf-8'],
  ['/logo.svg', '../../static/logo.svg', 'image/svg+xml']
].map(async([path,file,type])=>[path,{body:await readFile(new URL(file,import.meta.url)),type}])));
const owned=new Set();
let origin;
const server=createServer(async(req,res)=>{
  res.setHeader('Cache-Control','no-store');
  res.setHeader('X-Content-Type-Options','nosniff');
  res.setHeader('Content-Security-Policy',"default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'");
  const json=(status,data)=>{res.writeHead(status,{'Content-Type':'application/json'});res.end(JSON.stringify(data));};
  try {
    if(req.method==='GET'&&assets.has(req.url)){const asset=assets.get(req.url);res.writeHead(200,{'Content-Type':asset.type});res.end(asset.body);return;}
    if(!client){json(503,{error:{code:'NOT_CONFIGURED',message:'Configure BUNMAIL_API_URL, BUNMAIL_API_KEY, MAIL_FROM and MAIL_TO in the website backend'}});return;}
    if(req.method==='GET'&&req.url==='/connection'){const result=await client.check();json(200,{...result,from,to});return;}
    if(req.method==='GET'&&req.url.startsWith('/messages/')){
      const id=req.url.slice('/messages/'.length);
      if(!owned.has(id)){json(404,{error:{code:'NOT_FOUND',message:'Unknown message'}});return;}
      json(200,await client.get(id));return;
    }
    if(req.method!=='POST'||req.url!=='/send'){json(404,{error:{code:'NOT_FOUND',message:'Not found'}});return;}
    if(req.headers.origin!==origin){json(403,{error:{code:'INVALID_ORIGIN',message:'Invalid website origin'}});return;}
    if(!req.headers['content-type']?.startsWith('application/json')){json(415,{error:{code:'UNSUPPORTED_MEDIA_TYPE',message:'JSON required'}});return;}
    let size=0;const chunks=[];
    for await(const chunk of req){size+=chunk.length;if(size>16384){json(413,{error:{code:'PAYLOAD_TOO_LARGE',message:'Body too large'}});return;}chunks.push(chunk);}
    let input;
    try{input=JSON.parse(Buffer.concat(chunks).toString());}catch{json(400,{error:{code:'BAD_REQUEST',message:'Invalid JSON'}});return;}
    if(!input||typeof input.subject!=='string'||typeof input.text!=='string'||typeof input.key!=='string'||!input.subject.trim()||!input.text.trim()||input.key.length>128){json(422,{error:{code:'INVALID_INPUT',message:'Subject, text and idempotency key are required'}});return;}
    const result=await client.send({from,to,from_name:'Website integration',subject:input.subject,text:input.text},input.key);
    owned.add(result.data.id);json(result.status,result);
  }catch(error){
    // Never return environment variables, authorization headers or raw transport errors.
    json(error instanceof BunMailError&&error.status?error.status:502,{error:{code:error instanceof BunMailError?error.code:'REQUEST_FAILED',message:error instanceof BunMailError?error.message:'Request failed'},requestId:error.requestId||null,retryAfter:error.retryAfter||0});
  }
});
// Local demonstration only. Production websites must add their own authentication and abuse controls.
server.listen(Number(process.env.PORT??4000),'127.0.0.1',()=>{
  origin=`http://127.0.0.1:${server.address().port}`;
  process.stdout.write(JSON.stringify({url:origin})+'\n');
});
for(const signal of ['SIGTERM','SIGINT'])process.on(signal,()=>server.close(()=>process.exit(0)));
