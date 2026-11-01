export class BunMailError extends Error {
  constructor(message, {status=0, code='NETWORK_ERROR', requestId=null, retryAfter=0}={}) {
    super(message);
    Object.assign(this, {name:'BunMailError', status, code, requestId, retryAfter});
  }
}

// Node.js 22+; keep this client and its key in the website backend.
export class BunMailClient {
  #apiKey;
  constructor({baseUrl, apiKey, timeoutMs=10000}) {
    const url=new URL(baseUrl);
    if(url.username||url.password||url.search||url.hash||!['http:','https:'].includes(url.protocol))throw new Error('Invalid BunMail API URL');
    if(url.protocol==='http:'&&!['localhost','127.0.0.1','[::1]'].includes(url.hostname))throw new Error('HTTPS is required outside localhost');
    if(!apiKey)throw new Error('BUNMAIL_API_KEY is required');
    this.baseUrl=url.href.replace(/\/$/,'');this.#apiKey=apiKey;this.timeoutMs=timeoutMs;
  }
  async request(path,{method='GET',body,key}={}) {
    let response;
    try {
      response=await fetch(this.baseUrl+path,{
        method,credentials:'omit',redirect:'error',signal:AbortSignal.timeout(this.timeoutMs),
        headers:{Accept:'application/json',Authorization:`Bearer ${this.#apiKey}`,...(body?{'Content-Type':'application/json'}:{}),...(key?{'Idempotency-Key':key}:{})},
        ...(body?{body:JSON.stringify(body)}:{})
      });
    }catch {throw new BunMailError('Request failed or timed out; retry with the same idempotency key');}
    const requestId=response.headers.get('X-Request-ID');
    const retryAfter=Number(response.headers.get('Retry-After'))||0;
    let data;
    try {data=await response.json();}catch {throw new BunMailError('API returned a non-JSON response',{status:response.status,code:'INVALID_RESPONSE',requestId,retryAfter});}
    if(!response.ok)throw new BunMailError(data.error?.message||'API request failed',{status:response.status,code:data.error?.code||'REQUEST_FAILED',requestId,retryAfter});
    return {data,status:response.status,requestId};
  }
  check(){return this.request('/auth/check');}
  send(message,idempotencyKey){
    if(!idempotencyKey)throw new Error('Use a stable idempotency key for each business event');
    return this.request('/emails/send',{method:'POST',body:message,key:idempotencyKey});
  }
  get(id){return this.request(`/emails/${encodeURIComponent(id)}`);}
  list({page=0,status,search}={}){return this.request(`/emails?${new URLSearchParams({page:String(page),...(status?{status}:{}),...(search?{search}:{})})}`);}
}
