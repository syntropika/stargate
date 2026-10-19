/** Animate printable glyphs through the original mark, keeping its silhouette visible. */
export function initAsciiPortal(root: HTMLElement) {
  const canvas = root.querySelector('canvas');
  const toggle = root.querySelector<HTMLButtonElement>('.portal-toggle');
  const label = toggle?.querySelector('span');
  const fallback = root.querySelector<HTMLImageElement>('.portal-fallback');
  if (!canvas || !toggle || !label || !fallback) return;
  const ctx = canvas.getContext('2d');
  if (!ctx) return;

  const size = 540;
  const columns = 48;
  const step = 10.2;
  const offset = (size - columns * step) / 2;
  const glyphs = '01:+/=<>{}[]*#$%';
  const motion = window.matchMedia('(prefers-reduced-motion: reduce)');
  const sample = document.createElement('canvas');
  sample.width = sample.height = 96;
  const sampler = sample.getContext('2d', { willReadFrequently: true });
  if (!sampler) return;

  let mask: boolean[] = [];
  let ready = false;
  let visible = false;
  let paused = false;
  let disposed = false;
  let frame = 0;
  let lastTime = 0;
  let paintedTime = -Infinity;
  let elapsed = 0;
  const hash = (n: number) => {
    const value = Math.sin(n * 127.1 + 311.7) * 43758.5453;
    return value - Math.floor(value);
  };
  const streams = Array.from({ length: columns }, (_, x) => ({
    phase: hash(x + 1) * 68,
    speed: 4 + hash(x + 73) * 7,
    tail: 9 + Math.floor(hash(x + 151) * 10),
  }));

  function draw(seconds: number) {
    if (!ready || disposed || !ctx) return;
    ctx.clearRect(0, 0, size, size);
    ctx.font = '400 11.5px "Stargate Mono", monospace';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    for (let x = 0; x < columns; x++) {
      const stream = streams[x];
      const head = ((seconds * stream.speed + stream.phase) % (columns + stream.tail)) - 2;
      for (let y = 0; y < columns; y++) {
        const inside = mask[y * columns + x];
        const distance = head - y;
        const trail =
          distance >= 0 && distance < stream.tail ? Math.pow(1 - distance / stream.tail, 1.6) : 0;
        if (!inside && (motion.matches || trail < 0.2 || x % 3 !== 0)) continue;
        const base = inside ? 0.72 + hash(x * columns + y) * 0.16 : 0;
        const opacity = inside ? Math.min(1, base + trail * 0.5) : trail * 0.12;
        ctx.fillStyle =
          inside && trail > 0.68 ? `rgba(244,240,232,${opacity})` : `rgba(196,168,130,${opacity})`;
        const tick = Math.floor(seconds * (inside ? 3 : 6));
        const index = Math.floor(hash(x * 241 + y * 71 + tick) * glyphs.length);
        ctx.fillText(glyphs[index], offset + (x + 0.5) * step, offset + (y + 0.5) * step);
      }
    }
  }

  function animate(now: number) {
    frame = 0;
    if (disposed || !ready || !visible || paused || motion.matches || document.hidden) return;
    if (lastTime) elapsed += Math.min(now - lastTime, 100);
    lastTime = now;
    if (now - paintedTime >= 1000 / 24) {
      draw(elapsed / 1000);
      paintedTime = now;
    }
    frame = requestAnimationFrame(animate);
  }

  function sync() {
    cancelAnimationFrame(frame);
    frame = 0;
    lastTime = 0;
    if (disposed || !toggle || !label) return;
    toggle.hidden = motion.matches || !ready;
    toggle.setAttribute('aria-pressed', String(paused));
    toggle.setAttribute('aria-label', paused ? 'Resume logo animation' : 'Pause logo animation');
    label.textContent = paused ? 'Resume animation' : 'Pause animation';
    if (motion.matches) draw(0);
    if (ready && visible && !paused && !motion.matches && !document.hidden) {
      frame = requestAnimationFrame(animate);
    }
  }

  function onToggle() {
    paused = !paused;
    sync();
  }
  const observer = new IntersectionObserver(
    (entries) => {
      visible = entries[0].isIntersecting;
      sync();
    },
    { threshold: 0.05 },
  );
  observer.observe(root);
  toggle.addEventListener('click', onToggle);
  motion.addEventListener('change', sync);
  document.addEventListener('visibilitychange', sync);
  function onPageHide() {
    cancelAnimationFrame(frame);
    frame = 0;
    lastTime = 0;
  }
  window.addEventListener('pagehide', onPageHide);
  window.addEventListener('pageshow', sync);

  const image = new Image();
  image.onload = async () => {
    if (disposed) return;
    sampler.drawImage(image, 0, 0, 96, 96);
    const pixels = sampler.getImageData(0, 0, 96, 96).data;
    mask = Array.from({ length: columns * columns }, (_, i) => {
      const x = i % columns;
      const y = Math.floor(i / columns);
      return pixels[((y * 2 + 1) * 96 + x * 2 + 1) * 4 + 3] > 120;
    });
    try {
      await document.fonts.load('400 11.5px "Stargate Mono"');
      if (disposed) return;
      ready = true;
      canvas.hidden = false;
      fallback.hidden = true;
      draw(0);
      sync();
    } catch {
      // The original mark remains visible if the glyph font cannot be loaded.
    }
  };
  image.src = fallback.src;
  sync();

  // Kept explicit so a client-side host can dispose the effect before removing its surface.
  return () => {
    disposed = true;
    cancelAnimationFrame(frame);
    observer.disconnect();
    toggle.removeEventListener('click', onToggle);
    motion.removeEventListener('change', sync);
    document.removeEventListener('visibilitychange', sync);
    window.removeEventListener('pagehide', onPageHide);
    window.removeEventListener('pageshow', sync);
    image.onload = null;
  };
}
