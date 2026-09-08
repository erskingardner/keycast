import assert from 'node:assert/strict';
import diagnostics from 'node:diagnostics_channel';
import {readFileSync,writeFileSync,existsSync} from 'node:fs';
import {getPublicKey,finalizeEvent,verifyEvent} from 'nostr-tools/pure';
import {v2 as nip44} from 'nostr-tools/nip44';
import * as nip04 from 'nostr-tools/nip04';
import {BunkerSigner,parseBunkerInput} from 'nostr-tools/nip46';
// Run inside the disposable web image with /secrets and /results mounted.
// Requires pre-provisioned throwaway keys and a claimed grant; never use production keys.
const base=process.env.KEYCAST_TEST_API;
const source=process.env.KEYCAST_TEST_SOURCE;
assert(base && /^[a-f0-9]{40}$/.test(source ?? ''), 'Set test API and exact deployed source revision');
const raw=JSON.parse(readFileSync('/secrets/keys.json','utf8'));
const keys=Object.fromEntries(Object.entries(raw).map(([k,v])=>[k,Uint8Array.from(Buffer.from(v,'hex'))]));
const admin=getPublicKey(keys.admin), managed=getPublicKey(keys.managed), peer=getPublicKey(keys.peer);

console.debug=()=>{}; console.warn=()=>{};
const now=()=>Math.floor(Date.now()/1000);
// Node's WebSocket error hides failed upgrade statuses. Retain only HTTP status
// and time by public relay origin, never headers or request/response bodies.
const upgradeErrors=new Map();
diagnostics.channel('undici:request:headers').subscribe(({request,response})=>{
 if(response.statusCode>=400 && String(request.origin).startsWith('https://')) {
  upgradeErrors.set(new URL(request.origin).host,{status:response.statusCode,at:performance.now()});
 }
});
async function status() {
 const config=await fetch(`${base}/config?pubkey=${admin}`,{signal:AbortSignal.timeout(10000)}).then(r=>r.json());
 const event=finalizeEvent({kind:27237,created_at:now(),tags:[['u',base+'/status'],['method','GET'],['instance',config.instance_id]],content:''},keys.admin);
 const response=await fetch(base+'/status',{headers:{authorization:'Nostr '+Buffer.from(JSON.stringify(event)).toString('base64')},signal:AbortSignal.timeout(10000)});
 assert(response.ok, 'Status request failed');
 const s=await response.json(), m=s.signer;
 return {ready:m.ready,integrity_ok:m.integrity_ok,connected_relays:m.connected_relays,ingress_rejections:m.ingress_rejections,cached_retries:m.cached_retries,retry_coalesced:m.retry_coalesced,retry_throttled:m.retry_throttled,storage_rejections:m.storage_rejections,parse_errors:m.parse_errors,relay_failures:m.relay_failures,resources:m.resources,relays:s.relays.map(r=>({relay:new URL(r.url).origin,connection:r.diagnostics?.connection,subscription:r.diagnostics?.subscription,lifetime:r.reliability?.lifetime}))};
}
function errorClass(error) {
 if(String(error)==='Error: timeout'||error?.name==='TimeoutError') return 'timeout';
 if(error?.name==='AggregateError') return 'relay_publication_failed';
 if(error?.code==='ERR_ASSERTION') return 'assertion_failed';
 return 'operation_failed';
}
function relayReason(error) {
 const value=String(error).toLowerCase();
 if(value.includes('rate-limit')) return 'rate_limited';
 if(value.includes('duplicate')) return 'duplicate';
 if(value.includes('too old')||value.includes('too far')||value.includes('timestamp')) return 'timestamp';
 if(value.includes('timed out')||value.includes('timeout')) return 'timeout';
 if(value.includes('invalid')) return 'invalid';
 if(value.includes('auth-required')) return 'authentication_required';
 return 'other';
}
let signer; let phase='bootstrap'; let failures=0;
const results=[], publications=[], publicationWaits=[]; let closing=false; let before, after, failure;
function instrument(signer) {
 let activeMethod; const send=signer.sendRequest.bind(signer), publish=signer.pool.publish.bind(signer.pool);
 signer.sendRequest=(method,params)=>{activeMethod=method; return send(method,params);};
 signer.pool.publish=(relays,event,...args)=>{
  const method=activeMethod; const started=performance.now();
  const promises=publish(relays,event,...args).map((promise,i)=>promise.then(value=>{publications.push({method,event_id:event.id,relay:new URL(relays[i]).origin,ok:true,ms:Math.round(performance.now()-started)});return value;},error=>{const upgrade=upgradeErrors.get(new URL(relays[i]).host);const http_status=upgrade?.at>=started?upgrade.status:undefined;publications.push({method,event_id:event.id,relay:new URL(relays[i]).origin,ok:false,reason:closing?'client_closed':http_status===503?'service_unavailable':relayReason(error),phase:String(error).includes('connection failure:')?'connect':'publish',http_status,ms:Math.round(performance.now()-started)});throw error;}));
  publicationWaits.push(...promises); return promises;
 };
}
async function step(name,fn) {
 phase=name; const start=performance.now();
 let timer;
 try { await Promise.race([fn(),new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error('timeout')),30000)})]);
 const result={at:new Date().toISOString(),operation:name,ok:true,ms:Math.round(performance.now()-start)};results.push(result);console.log(JSON.stringify(result));
 } catch(error) {results.push({at:new Date().toISOString(),operation:name,ok:false,ms:Math.round(performance.now()-start),error_class:errorClass(error)});throw error;} finally {clearTimeout(timer);}
}
async function denied(fn,pattern) {
 let denied=false;
 try {await fn();} catch(error) {denied=pattern.test(String(error));}
 assert(denied,'Expected explicit denial, not success or transport timeout');
}
try {
 const state=JSON.parse(readFileSync('/secrets/state.json','utf8'));
 before=await status();
 const bp=await parseBunkerInput(state.bunker); assert(bp);
 signer=BunkerSigner.fromBunker(keys.client,bp); instrument(signer);
 if(!state.connected) {
  await step('connect',()=>signer.connect()); state.connected=true;
  writeFileSync('/secrets/state.json',JSON.stringify(state),{mode:0o600});
 }
 await step('ping',()=>signer.ping());
 await step('switch_relays',async()=>{const relays=JSON.parse(await signer.sendRequest('switch_relays',[]));assert(Array.isArray(relays)&&relays.length>0&&relays.length<=20);await signer.switchRelays();});
 await step('get_public_key',async()=>assert.equal(await signer.getPublicKey(),managed));
 await step('sign_event',async()=>{
  const event=await signer.signEvent({kind:1,created_at:now(),tags:[],content:'Disposable Keycast VM signing test; not published.'});
  assert(verifyEvent(event));assert.equal(event.pubkey,managed);
 });
 const message='Keycast disposable encryption test — café 日本語 🔑';
 await step('nip04_encrypt',async()=>assert.equal(await nip04.decrypt(keys.peer,managed,await signer.nip04Encrypt(peer,message)),message));
 await step('nip04_decrypt',async()=>assert.equal(await signer.nip04Decrypt(peer,await nip04.encrypt(keys.peer,managed,message)),message));
 const conv=nip44.utils.getConversationKey(keys.peer,managed);
 await step('nip44_encrypt',async()=>assert.equal(nip44.decrypt(await signer.nip44Encrypt(peer,message),conv),message));
 await step('nip44_decrypt',async()=>assert.equal(await signer.nip44Decrypt(peer,nip44.encrypt(message,conv)),message));
 await step('self_encrypt_decrypt',async()=>assert.equal(await signer.nip44Decrypt(managed,await signer.nip44Encrypt(managed,message)),message));
 await step('deny_event_kind',()=>denied(()=>signer.signEvent({kind:7,created_at:now(),tags:[],content:'+'}),/policy denied/));
 await step('reject_invalid_ciphertext',()=>denied(()=>signer.nip44Decrypt(peer,'invalid ciphertext'),/cryptographic operation failed/));
 await step('reject_invalid_recipient',()=>denied(()=>signer.nip04Encrypt('bad-key',message),/invalid public key/));
 await step('reject_unknown_method',()=>denied(()=>signer.sendRequest('unsupported_method',[]),/unsupported|unknown|not supported/));
 await step('relay_acknowledgments',()=>Promise.allSettled(publicationWaits));
 closing=true; await signer.close(); signer.pool.destroy();
 // A separate fixture exercises automatically discovered destinations only,
 // so baseline availability cannot hide a broken discovered-relay connection.
 if(existsSync('/secrets/discovery-state.json')) {
  const discovered=JSON.parse(readFileSync('/secrets/discovery-state.json','utf8'));
  const pointer=await parseBunkerInput(discovered.bunker); assert(pointer);
  const client=Uint8Array.from(Buffer.from(discovered.client,'hex'));
  assert(Array.isArray(discovered.relays)&&discovered.relays.length>0&&discovered.relays.every(r=>pointer.relays.includes(r)));
  signer=BunkerSigner.fromBunker(client,{...pointer,relays:discovered.relays});
  instrument(signer); closing=false;
  await step('discovered_relay_ping',()=>signer.ping());
  await step('discovered_relay_sign',async()=>{const event=await signer.signEvent({kind:1,created_at:now(),tags:[],content:'Disposable discovered-relay test; not published.'});assert(verifyEvent(event));assert.equal(event.pubkey,discovered.pub);});
  await step('discovered_relay_nip04',async()=>{
   assert.equal(await nip04.decrypt(keys.peer,discovered.pub,await signer.nip04Encrypt(peer,message)),message);
   assert.equal(await signer.nip04Decrypt(peer,await nip04.encrypt(keys.peer,discovered.pub,message)),message);
  });
  await step('discovered_relay_nip44',async()=>{
   const conv=nip44.utils.getConversationKey(keys.peer,discovered.pub);
   assert.equal(nip44.decrypt(await signer.nip44Encrypt(peer,message),conv),message);
   assert.equal(await signer.nip44Decrypt(peer,nip44.encrypt(message,conv)),message);
  });
  await step('discovered_relay_acknowledgments',()=>Promise.allSettled(publicationWaits));
  closing=true; await signer.close(); signer.pool.destroy();client.fill(0);
 }
 after=await status();
 assert(after.ready && after.integrity_ok, 'Signer must remain ready with valid integrity');
} catch(error) {
 failures++; failure={phase,error_class:errorClass(error)};
 try {after=await status();} catch {}
} finally {
 closing=true; if(signer){await signer.close();signer.pool.destroy();}
 for(const key of Object.values(keys)) key.fill(0);
 const summary={at:new Date().toISOString(),source,result:failures?'FAIL':'PASS',failure,results,publications,before,after};
 writeFileSync('/results/latest.json',JSON.stringify(summary,null,2),{mode:0o600});
 console.log(JSON.stringify({result:summary.result,operations:results.length,failure,ingress_delta:after&&before?after.ingress_rejections-before.ingress_rejections:null}));
 process.exit(failures?1:0);
}
