import test from 'node:test';
import assert from 'node:assert/strict';
import { purchaseAvailable } from './sales-gate.mjs';
const ready = {signedMacRelease:true, downloadAndMicrophoneTested:true, upgradeTested:true,
  sellerDisclosurePublished:true, supportAndRefundPolicyPublished:true};
const store = {name:'Gumroad',enabled:true,url:'https://shiyou5.gumroad.com/l/minutespro'};
test('all launch checks and enabled store required', () => {
  assert.equal(purchaseAvailable(store, ready), true);
  assert.equal(purchaseAvailable({...store,enabled:false}, ready), false);
  for (const key of Object.keys(ready)) assert.equal(purchaseAvailable(store,{...ready,[key]:false}), false);
  assert.equal(purchaseAvailable(store, null), false);
});
test('malformed, insecure, lookalike and credential URLs fail closed', () => {
  for (const url of ['no',null,'http://shiyou5.gumroad.com/l/x', 'https://shiyou5.gumroad.com.evil.test/x',
    'https://user:pass@shiyou5.gumroad.com/l/x','javascript:alert(1)'])
    assert.equal(purchaseAvailable({...store,url}, ready), false);
});
test('alternative stores supported only on their own hosts', () => {
  assert.equal(purchaseAvailable({name:'Payhip',enabled:true,url:'https://payhip.com/b/sample'}, ready), true);
  assert.equal(purchaseAvailable({name:'BOOTH',enabled:true,url:'https://sample.booth.pm/items/1'}, ready), true);
  assert.equal(purchaseAvailable({...store,name:'unknown'}, ready), false);
});
