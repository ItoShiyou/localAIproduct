(() => {
  const holder = document.querySelector('.bird-float');
  if (!holder) return;
  const still = holder.querySelector('img');
  const reduced = matchMedia('(prefers-reduced-motion: reduce)');
  const button = document.createElement('button');
  button.type = 'button';
  button.className = 'bird-button';
  button.setAttribute('aria-label', '小鳥にあいさつする');
  button.title = 'こんにちは。';
  holder.closest('.hero-art').removeAttribute('aria-hidden');
  holder.closest('.hero-art').querySelectorAll('.orbit, .paper-note, .voice-line').forEach(el => el.setAttribute('aria-hidden', 'true'));
  button.append(still);
  holder.append(button);
  still.dataset.pose = 'still';
  still.classList.add('bird-frame', 'is-active');
  const frames = new Map([['still', still]]);
  let ready = false;
  let visible = false;
  let timer;
  let frameTimer;
  let active = false;
  let cycle = 0;
  const allowed = () => ready && visible && !document.hidden && !reduced.matches && !document.documentElement.classList.contains('motion-off');
  const show = pose => frames.forEach((image, name) => image.classList.toggle('is-active', name === pose));
  function reset() {
    clearTimeout(timer);
    clearTimeout(frameTimer);
    active = false;
    button.classList.remove('is-greeting');
    show('still');
  }
  function nextIdle() {
    clearTimeout(timer);
    if (!allowed()) return;
    timer = setTimeout(() => play(cycle++ % 3 === 2 ? 'tilt' : 'blink'), 7000 + (cycle % 3) * 1800);
  }
  function play(pose) {
    if (!allowed() || active) return;
    clearTimeout(timer);
    active = true;
    show(pose);
    if (pose === 'wave') button.classList.add('is-greeting');
    frameTimer = setTimeout(() => {
      show('still');
      button.classList.remove('is-greeting');
      active = false;
      nextIdle();
    }, pose === 'blink' ? 180 : pose === 'wave' ? 650 : 850);
  }
  button.addEventListener('click', () => play('wave'));
  button.addEventListener('pointerenter', event => { if (event.pointerType === 'mouse') play('wave'); });
  function sync() {
    reset();
    holder.style.animationPlayState = visible && !document.hidden && !reduced.matches && !document.documentElement.classList.contains('motion-off') ? 'running' : 'paused';
    nextIdle();
  }
  document.addEventListener('visibilitychange', sync);
  document.addEventListener('minutes-motion-change', sync);
  reduced.addEventListener('change', sync);
  if ('IntersectionObserver' in window) {
    const observer = new IntersectionObserver(entries => { visible = entries[0].isIntersecting; sync(); }, { threshold: .15 });
    observer.observe(button);
  } else visible = true;
  Promise.all(['wave', 'blink', 'tilt'].map(pose => new Promise(resolve => {
    const image = new Image();
    image.alt = '';
    image.className = 'mascot bird-frame';
    image.dataset.pose = pose;
    image.width = still.width;
    image.height = still.height;
    image.onload = () => { frames.set(pose, image); button.append(image); resolve(true); };
    image.onerror = () => resolve(false);
    image.src = `../assets/minutes-bird-${pose}.png`;
  }))).then(results => { ready = results.every(Boolean); sync(); });
})();
