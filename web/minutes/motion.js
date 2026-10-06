(() => {
  const root = document.documentElement;
  const reduced = matchMedia('(prefers-reduced-motion: reduce)');
  const toggle = document.querySelector('.motion-toggle');
  const preview = document.querySelector('[data-parallax]');
  let stopped = false;
  let scheduled = false;
  function update() {
    scheduled = false;
    const limit = root.scrollHeight - innerHeight;
    root.style.setProperty('--scroll-progress', String(limit > 0 ? Math.min(1, Math.max(0, scrollY / limit)) : 0));
    if (preview && !stopped && !reduced.matches) {
      const rect = preview.getBoundingClientRect();
      const progress = Math.min(1, Math.max(0, (innerHeight - rect.top) / innerHeight));
      preview.style.setProperty('--preview-tilt', `${(1 - progress) * 5}deg`);
      preview.style.setProperty('--preview-scale', String(.97 + progress * .03));
    }
  }
  function schedule() { if (!scheduled) { scheduled = true; requestAnimationFrame(update); } }
  if ('IntersectionObserver' in window) {
    const observer = new IntersectionObserver(entries => entries.forEach(entry => {
      if (entry.isIntersecting) { entry.target.classList.add('is-visible'); observer.unobserve(entry.target); }
    }), { threshold: .08 });
    document.querySelectorAll('.reveal').forEach(element => observer.observe(element));
    root.classList.add('motion-ready');
  }
  toggle.hidden = false;
  function setMotion() {
    root.classList.toggle('motion-off', stopped || reduced.matches);
    toggle.textContent = stopped ? '動きを戻す' : '動きを止める';
    toggle.setAttribute('aria-pressed', String(stopped));
    document.dispatchEvent(new Event('minutes-motion-change'));
    schedule();
  }
  toggle.addEventListener('click', () => { stopped = !stopped; setMotion(); });
  reduced.addEventListener('change', setMotion);
  addEventListener('scroll', schedule, { passive: true });
  addEventListener('resize', schedule);
  setMotion();
})();
