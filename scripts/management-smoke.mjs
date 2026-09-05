// Runs inside the disposable web image. This fixed key is test data only.
import assert from 'node:assert/strict';
import {createHash,randomBytes} from 'node:crypto';
import net from 'node:net';
import {getPublicKey,finalizeEvent,generateSecretKey} from 'nostr-tools/pure';
import {v2 as nip44} from 'nostr-tools/nip44';
const key=new Uint8Array(32).fill(11), pubkey=getPublicKey(key);
const base='http://api:3000/api', canonical='https://keycast.test/api';
const hash=body=>createHash('sha256').update(body).digest('hex');
const responseKey=generateSecretKey();
async function http(url, options = {}) {
 const controller = new AbortController();
 const timer = setTimeout(() => controller.abort(), 15000);
 try {
  const response = await fetch(url, {...options, signal: controller.signal});
  // Include body consumption in the deadline, since headers may arrive before a stalled body.
  return new Response(await response.arrayBuffer(), {status: response.status, headers: response.headers});
 } finally { clearTimeout(timer); }
}
async function request(method,path,data,legacy=false) {
 const body=data===undefined?'':JSON.stringify(data);
 const config=await http(`${base}/config?pubkey=${pubkey}`).then(r=>r.json());
 const tags=[['u',canonical+path],['method',method]];
 const write=method!=='GET';
 if(write) tags.push(['payload',hash(body)]);
 if(!legacy) tags.push(['instance',config.instance_id]);
 if(write&&!legacy) tags.push(['revision',String(config.authority_revision)],['nonce',randomBytes(32).toString('hex')],['response',getPublicKey(responseKey)]);
 const content=write&&!legacy?(method==='POST'&&path.endsWith('/keys')?`Import private key named ${data.name}`:body):'';
 const event=finalizeEvent({kind:legacy?27235:write?27236:27237,created_at:Math.floor(Date.now()/1000),tags,content},key);
 const response=await http(base+path,{method,headers:{authorization:'Nostr '+Buffer.from(JSON.stringify(event)).toString('base64'),'content-type':'application/json'},body:write&&body?body:undefined});
 if(write&&!legacy&&response.ok) {
  const encrypted=await response.json();assert.ok(encrypted.encrypted_response);assert.ok(!JSON.stringify(encrypted).includes('bunker://'));
  const reply=JSON.parse(nip44.decrypt(encrypted.encrypted_response,nip44.utils.getConversationKey(responseKey,encrypted.public_key)));
  return {status:reply.status,body:reply.body?JSON.parse(reply.body):null};
 }
 return {status:response.status,body:await response.json().catch(()=>null)};
}
assert.equal((await http('http://api:3000/health')).status,200);
assert.equal((await http('http://web:5173/health')).status,200);
assert.equal((await http('http://api:3000/ready')).status,200);
assert.equal((await request('POST','/teams',{name:'Must not exist'},true)).status,401);
const team=await request('POST','/teams',{name:'Smoke'});assert.equal(team.status,201);
assert.equal((await request('PUT','/relays',{minimum_connected_relays:1,relays:[{url:'ws://127.0.0.1:1',enabled:true}]})).status,200);
const imported=await request('POST',`/teams/${team.body.team.id}/keys`,{name:'Disposable',secret_key:Buffer.from(key).toString('hex')});assert.equal(imported.status,201);
const policy=await request('POST',`/teams/${team.body.team.id}/policies`,{name:'Notes',document:{version:1,capabilities:{sign_event:{allowed_kinds:[1]}}}});assert.equal(policy.status,201);
const grant=await request('POST',`/teams/${team.body.team.id}/keys/${pubkey}/grants`,{name:'Disposable',policy_id:policy.body.id,invitation_expires_at:Math.floor(Date.now()/1000)+300});assert.equal(grant.status,201);assert.ok(grant.body.bunker_uri.startsWith('bunker://'));
const teams=await request('GET','/teams');assert.equal(teams.status,200);
const legacyResult=await new Promise((resolve,reject)=>{
 const socket=net.connect('/run/keycast/signer.sock');let body='';
 socket.setTimeout(5000,()=>socket.destroy(new Error('control test timed out')));
 socket.on('error',reject);socket.on('data',chunk=>body+=chunk);socket.on('end',()=>resolve(JSON.parse(body)));
 socket.on('connect',()=>socket.end(JSON.stringify({operation:'create_grant',team_id:team.body.team.id,actor_public_key:pubkey,stored_key_id:imported.body.id,policy_id:policy.body.id,name:'Forged authority',expires_at:null,invitation_expires_at:Math.floor(Date.now()/1000)+300})));
});
assert.equal(legacyResult.result,'error');assert.equal(legacyResult.code,'invalid_request');
key.fill(0);responseKey.fill(0);
console.log('PASS: read-only API/signer/web; signed management; legacy write rejection; browser key import; encrypted invitation creation');
