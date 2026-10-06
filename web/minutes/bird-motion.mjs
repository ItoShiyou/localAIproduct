export const MOTIONS = Object.freeze({
  breathe: { label: 'ひと息', duration: 1800, frames: [[0, 'still']] },
  wave: { label: 'こんにちは', duration: 1100, frames: [[0, 'still'], [160, 'wave'], [850, 'still']] },
  blink: { label: 'まばたき', duration: 450, frames: [[0, 'still'], [120, 'blink'], [280, 'still']] },
  tilt: { label: '耳をかたむける', duration: 1300, frames: [[0, 'still'], [180, 'tilt'], [1050, 'still']] },
  nod: { label: 'うん、うん', duration: 1000, frames: [[0, 'still']] },
  settle: { label: 'おつかれさま', duration: 1400, frames: [[0, 'still'], [250, 'blink'], [1000, 'still']] },
});

export function createMotionPlayer({ render, schedule = setTimeout, cancel = clearTimeout }) {
  let timers = [];
  let generation = 0;
  function stop() {
    generation++;
    timers.forEach(cancel);
    timers = [];
    render('still', 'rest');
  }
  function play(name, allowed = () => true, done = () => {}) {
    stop();
    if (!Object.hasOwn(MOTIONS, name) || !allowed()) return false;
    const token = generation;
    const motion = MOTIONS[name];
    const guarded = callback => {
      if (token !== generation) return;
      if (!allowed()) { stop(); return; }
      callback();
    };
    motion.frames.forEach(([time, pose]) => {
      if (time === 0) render(pose, name);
      else timers.push(schedule(() => guarded(() => render(pose, name)), time));
    });
    timers.push(schedule(() => guarded(() => { stop(); done(); }), motion.duration));
    return true;
  }
  return { play, stop };
}
