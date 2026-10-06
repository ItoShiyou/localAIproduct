import { MOTIONS, createMotionPlayer } from './bird-motion.mjs';

document.querySelectorAll('[data-koyori]').forEach(holder => {
  const still = holder.querySelector('img');
  if (!still) return;
  const reduced = matchMedia('(prefers-reduced-motion: reduce)');
  const button = document.createElement('button');
  button.type = 'button';
  button.className = 'bird-button';
  button.setAttribute('aria-label', 'こよりにあいさつする');
  button.title = 'こんにちは。こよりです。';
  button.append(still);
  holder.append(button);
  still.alt = '';
  still.dataset.pose = 'still';
  still.classList.add('bird-frame', 'is-active');
  const frames = new Map([['still', still]]);
  const scope = holder.closest('[data-koyori-stage]') || holder;
  const controls = scope.querySelector('[data-koyori-controls]');
  const status = scope.querySelector('[data-koyori-status]');
  let ready = false;
  let visible = false;
  let idleTimer;
  let cycle = 0;
  const allowed = () => ready && visible && !document.hidden && !reduced.matches && !document.documentElement.classList.contains('motion-off');
  const player = createMotionPlayer({ render(pose, motion) {
    const selected = frames.has(pose) ? pose : 'still';
    frames.forEach((image, name) => image.classList.toggle('is-active', name === selected));
    button.dataset.motion = motion;
  } });
  function nextIdle() {
    clearTimeout(idleTimer);
    if (!allowed() || holder.dataset.koyoriAuto === 'false') return;
    idleTimer = setTimeout(() => player.play(cycle++ % 3 === 2 ? 'tilt' : 'blink', allowed, nextIdle), 7500 + (cycle % 3) * 1700);
  }
  function play(name) {
    if (!Object.hasOwn(MOTIONS, name)) return;
    clearTimeout(idleTimer);
    if (!player.play(name, allowed, nextIdle)) {
      if (status) status.textContent = '動きを止めています。静かなこよりをどうぞ。';
      return;
    }
    if (status) status.textContent = MOTIONS[name].label;
    controls?.querySelectorAll('[data-koyori-motion]').forEach(control => control.setAttribute('aria-pressed', String(control.dataset.koyoriMotion === name)));
  }
  button.addEventListener('click', () => play('wave'));
  controls?.querySelectorAll('[data-koyori-motion]').forEach(control => control.addEventListener('click', () => play(control.dataset.koyoriMotion)));
  function sync() {
    clearTimeout(idleTimer);
    player.stop();
    button.classList.toggle('is-still', !allowed());
    if (status) status.textContent = allowed() ? 'こよりを押すと、あいさつします。' : '動きを止めています。静かなこよりをどうぞ。';
    controls?.querySelectorAll('[data-koyori-motion]').forEach(control => control.setAttribute('aria-pressed', 'false'));
    nextIdle();
  }
  document.addEventListener('visibilitychange', sync);
  document.addEventListener('minutes-motion-change', sync);
  reduced.addEventListener('change', sync);
  if ('IntersectionObserver' in window) {
    const observer = new IntersectionObserver(entries => { visible = entries[0].isIntersecting; sync(); }, { threshold: .15 });
    observer.observe(button);
  } else visible = true;
  const originalUrl = new URL(still.getAttribute('src'), document.baseURI);
  Promise.all(['wave', 'blink', 'tilt'].map(pose => new Promise(resolve => {
    const image = new Image();
    image.alt = '';
    image.className = 'mascot bird-frame';
    image.dataset.pose = pose;
    image.width = still.width;
    image.height = still.height;
    image.onload = () => { frames.set(pose, image); button.append(image); resolve(); };
    image.onerror = () => resolve(); // Missing variants never hide the original.
    image.src = new URL(`minutes-bird-${pose}.png`, originalUrl).href;
  }))).then(() => { ready = true; if (controls) controls.hidden = false; sync(); });
});
