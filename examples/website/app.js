'use strict';
const form=document.querySelector('#send-form'),button=form.querySelector('button'),result=document.querySelector('#result');
let key=crypto.randomUUID(),payload='',messageId=null;
async function api(path,options){const response=await fetch(path,options);const data=await response.json();if(!response.ok)throw data;return data;}
function show(data){result.hidden=false;document.querySelector('#result-text').textContent=JSON.stringify(data,null,2);}
api('/connection').then(data=>{document.querySelector('#connection').textContent='连接正常';document.querySelector('#from').textContent=data.from;document.querySelector('#to').textContent=data.to;button.disabled=false;}).catch(error=>{document.querySelector('#connection').textContent='连接失败';show(error);});
form.addEventListener('submit',async event=>{
  event.preventDefault();if(button.disabled)return;button.disabled=true;
  const values=Object.fromEntries(new FormData(form)),current=JSON.stringify(values);
  if(payload!==current){key=crypto.randomUUID();payload=current;}
  try{const data=await api('/send',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({...values,key})});messageId=data.data.id;show(data);document.querySelector('#refresh').hidden=false;}
  catch(error){show(error);}
  finally{button.disabled=false;}
});
document.querySelector('#refresh').addEventListener('click',async event=>{event.target.disabled=true;try{show(await api('/messages/'+encodeURIComponent(messageId)));}catch(error){show(error);}finally{event.target.disabled=false;}});
