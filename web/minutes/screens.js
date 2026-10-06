(() => {
  const screens = {
    read: ['読む', '全文を段落で読み、会話の流れをつかむ。まず内容を知りたいときの画面です。', 'reading'],
    edit: ['聞いて直す', '音声を聞きながら、文字と話者を修正。文の分割・結合や要確認の見直しも、ここで。', 'edit'],
    notes: ['議事録を仕上げる', '元の会話を参照しながら、決定事項やToDoを整理。要約は確認して使う下書きです。', 'minutes'],
    focus: ['一文ずつ直す', '一度に見るのは一文だけ。短く聞き直し、ひとつずつ修正して次へ進みます。', 'focus'],
  };
  const tabs = Array.from(document.querySelectorAll('[data-screen]'));
  const panel = document.getElementById('screen-panel');
  function activate(tab, focus = false) {
    const [title, description, image] = screens[tab.dataset.screen];
    tabs.forEach(item => { item.setAttribute('aria-selected', String(item === tab)); item.tabIndex = item === tab ? 0 : -1; });
    panel.setAttribute('aria-labelledby', tab.id);
    document.getElementById('screen-title').textContent = title;
    document.getElementById('screen-description').textContent = description;
    document.getElementById('screen-image').src = `../assets/minutes-calm-${image}.jpg`;
    document.getElementById('screen-image').alt = `minutesの${title}画面`;
    if (!matchMedia('(prefers-reduced-motion: reduce)').matches && !document.documentElement.classList.contains('motion-off')) {
      panel.animate([{ opacity: .65, transform: 'translateY(5px)' }, { opacity: 1, transform: 'translateY(0)' }], { duration: 300, easing: 'ease-out' });
    }
    if (focus) tab.focus();
  }
  tabs.forEach((tab, index) => {
    tab.addEventListener('click', () => activate(tab));
    tab.addEventListener('keydown', event => {
      const next = event.key === 'ArrowRight' ? (index + 1) % tabs.length : event.key === 'ArrowLeft' ? (index + tabs.length - 1) % tabs.length : event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1 : null;
      if (next !== null) { event.preventDefault(); activate(tabs[next], true); }
    });
  });
  activate(tabs[0]);
  document.querySelector('.tabs').hidden = false;
})();
