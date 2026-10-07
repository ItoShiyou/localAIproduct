try {
  const response = await fetch('./stores.json', { cache: 'no-cache' });
  if (!response.ok) throw new Error('Unavailable');
  const stores = await response.json();
  const root = document.getElementById('stores');
  root.replaceChildren();
  for (const store of stores) {
    const card = document.createElement('section');
    const title = document.createElement('h2'); title.textContent = store.name;
    const status = document.createElement('p'); status.className = 'status'; status.textContent = store.status;
    card.append(title, status);
    if (store.enabled && typeof store.url === 'string' && new URL(store.url).protocol === 'https:') {
      const link = document.createElement('a'); link.href = store.url; link.rel = 'noopener noreferrer';
      link.textContent = `${store.name}で購入する ↗`; card.append(link);
    }
    root.append(card);
  }
} catch {
  document.getElementById('stores').replaceChildren();
  document.getElementById('error').hidden = false;
}
