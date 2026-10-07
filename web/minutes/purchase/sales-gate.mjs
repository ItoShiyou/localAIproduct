const required = ['signedMacRelease', 'downloadAndMicrophoneTested', 'upgradeTested',
  'sellerDisclosurePublished', 'supportAndRefundPolicyPublished'];
const hosts = { Gumroad: ['shiyou5.gumroad.com'], Payhip: ['payhip.com'], BOOTH: ['booth.pm'] };
export function purchaseAvailable(store, readiness) {
  if (!store || store.enabled !== true || !required.every(key => readiness?.[key] === true)) return false;
  try {
    const url = new URL(store.url);
    return url.protocol === 'https:' && !url.username && !url.password &&
      hosts[store.name]?.some(host => url.hostname === host || url.hostname.endsWith(`.${host}`)) === true;
  } catch { return false; }
}
