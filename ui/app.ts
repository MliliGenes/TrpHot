import { BoxRenderable, TextRenderable, type CliRenderer, type KeyEvent } from '@opentui/core';

export type Config = { targets: string[]; makefile: string; exec: string; ignores: string[]; interval: number; debounce: number };
type File = { path: string; size: number; ignored: boolean };

const clean = (s: unknown) => String(s ?? '').replace(/\x1b\][^\x07]*(?:\x07|\x1b\\)/g, '').replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, '').replace(/[\x00-\x08\x0b-\x1f\x7f]/g, '');

export function mount(renderer: CliRenderer, send: (v: any) => void, quit: () => void) {
  // Tokyo Night-inspired colors, with the same restrained contrast that makes
  // LazyVim comfortable for long sessions in a terminal.
  const C = {
    bg: '#0f111a',
    surface: '#161a24',
    panel: '#131720',
    panelRaised: '#1b2030',
    border: '#2b3245',
    borderActive: '#565f89',
    text: '#c0caf5',
    dim: '#737a9c',
    faint: '#4b526d',
    blue: '#7aa2f7',
    cyan: '#7dcfff',
    purple: '#bb9af7',
    orange: '#ff9e64',
    green: '#9ece6a',
    yellow: '#e0af68',
    red: '#f7768e',
  };

  const root = new BoxRenderable(renderer, {
    width: '100%', height: '100%', flexDirection: 'column', backgroundColor: C.bg, paddingX: 1,
  });
  renderer.root.add(root);

  const brandBar = new BoxRenderable(renderer, {
    height: 3, flexDirection: 'column', backgroundColor: C.surface, paddingX: 1,
  });
  const brand = new TextRenderable(renderer, { height: 1, content: '  TRPHOT  /  PROJECT WATCHER', fg: C.blue, bg: C.surface });
  const breadcrumb = new TextRenderable(renderer, { height: 1, content: '  WATCH  ·  BUILD  ·  RESTART', fg: C.dim, bg: C.surface });
  brandBar.add(brand); brandBar.add(breadcrumb); root.add(brandBar);

  const statusBar = new BoxRenderable(renderer, {
    height: 1, flexDirection: 'row', backgroundColor: C.surface,
  });
  const mode = new TextRenderable(renderer, { width: 15, height: 1, content: ' SPECTATOR ', fg: C.bg, bg: C.blue });
  const summary = new TextRenderable(renderer, { flexGrow: 1, height: 1, content: '', fg: C.text, bg: C.surface });
  statusBar.add(mode); statusBar.add(summary); root.add(statusBar);

  const tabs = new TextRenderable(renderer, { height: 2, content: '', fg: C.dim, bg: C.bg });
  root.add(tabs);

  const panel = new BoxRenderable(renderer, {
    flexGrow: 1,
    border: true,
    borderStyle: 'rounded',
    borderColor: C.border,
    focusedBorderColor: C.borderActive,
    title: ' Files ',
    titleColor: C.blue,
    bottomTitle: ' q quit ',
    bottomTitleAlignment: 'right',
    backgroundColor: C.panel,
    paddingX: 1,
  });
  root.add(panel);
  const body = new TextRenderable(renderer, { width: '100%', height: '100%', content: '', fg: C.text, bg: C.panel, wrapMode: 'none' });
  panel.add(body);

  const prompt = new TextRenderable(renderer, { height: 1, content: '', fg: C.orange, bg: C.bg });
  root.add(prompt);

  const footer = new TextRenderable(renderer, { height: 3, content: '', fg: C.dim, bg: C.surface, wrapMode: 'none' });
  root.add(footer);

  let files: File[] = [];
  let cfg: Config | undefined;
  let selected = 0;
  let page = 0;
  let logOffset = 0;
  let active = false;
  let paused = false;
  let building = false;
  let rootPath = '';
  let message = '';
  let edit: keyof Config | 'ignore' | undefined;
  let ruleOffset = 0;
  let search = '';
  let allFiles: File[] = [];
  let searching = false;
  let editing = false;
  let inputValue = '';
  const logs: string[] = [];

  function log(s: string) {
    logs.push(clean(s));
    if (logs.length > 2000) logs.splice(0, logs.length - 2000);
  }

  function filter() {
    files = allFiles.filter(f => f.path.toLowerCase().includes(search.toLowerCase()));
  }

  function draw() {
    // Fixed chrome is 12 rows; leave the rest to the active view.
    const rows = Math.max(1, renderer.height - 12);
    const width = Math.max(10, renderer.width - 8);
    const watched = files.filter(f => !f.ignored).length;
    const modeLabel = building ? ' BUILDING ' : paused ? ' PAUSED ' : active ? ' NORMAL ' : ' SPECTATOR ';

    brand.content = `  TRPHOT  /  ${clean(rootPath)}`;
    breadcrumb.content = `  WATCH  ·  BUILD  ·  RESTART${search ? `  ·  FILTER: ${clean(search)}` : ''}`;
    mode.content = modeLabel;
    mode.bg = building ? C.orange : paused ? C.yellow : active ? C.green : C.blue;
    summary.content = `  ${watched} watched  ·  ${files.length} visible${message ? `  ·  ${clean(message)}` : '  ·  Ready'}`;

    const tabNames = ['FILES', 'ACTIVITY', 'RULES'];
    tabs.content = `  ${tabNames.map((name, i) => i === page ? `[ ${name} ]` : `  ${name}  `).join('   ')}\n  ${'─'.repeat(Math.max(1, Math.min(width - 2, 68)))}`;
    panel.title = ` ${tabNames[page]} `;
    panel.titleColor = page === 0 ? C.blue : page === 1 ? C.purple : C.cyan;

    if (page === 0) {
      selected = Math.max(0, Math.min(selected, Math.max(0, files.length - 1)));
      const start = Math.max(0, Math.min(selected - Math.floor(rows / 2), Math.max(0, files.length - rows)));
      body.content = files.length
        ? files.slice(start, start + rows).map((f, i) => {
          const cursor = start + i === selected ? '▸' : ' ';
          const kind = f.ignored ? '○ ignore' : '● watch ';
          return `${cursor} ${kind}  ${clean(f.path)}  ${f.size} B`;
        }).map(line => line.slice(0, width)).join('\n')
        : '  No visible files. Add source files or review your ignore rules.';
    } else if (page === 1) {
      logOffset = Math.max(0, Math.min(logOffset, Math.max(0, logs.length - rows)));
      body.content = logs.slice(Math.max(0, logs.length - rows - logOffset), logs.length - logOffset)
        .map(x => `  ${x}`.slice(0, width)).join('\n') || '  File events and process output will appear here.';
    } else if (cfg) {
      const lines = [
        '  Project rules',
        '  ─────────────',
        `  t  Targets       ${cfg.targets.join(' ')}`,
        `  m  Makefile      ${cfg.makefile}`,
        `  e  Execute       ${cfg.exec || '(build only)'}`,
        `  i  Poll          ${cfg.interval} ms`,
        `  d  Debounce      ${cfg.debounce} ms`,
        '',
        '  g  Add ignore glob       c  Clear custom ignores',
        '  s  Save current rules to .trphot.json',
        '',
        '  Ignored globs:',
        ...cfg.ignores.map(p => `    ${clean(p)}`),
        '',
        '  Always excluded: .git, node_modules, target, .trphot.json',
        '  Symlinks are not followed.',
      ];
      body.content = lines.slice(ruleOffset, ruleOffset + rows).map(x => x.slice(0, width)).join('\n');
    }

    footer.content = [
      '  [Tab] view   [↑↓] navigate   [/] filter   [x] ignore',
      '  [a] arm   [b] build   [p] pause   [s] save   [q] quit',
      `  ${clean(rootPath) || 'project'}  ·  OpenTUI cockpit`,
    ].join('\n');
  }

  function begin(field: keyof Config | 'ignore' | 'search') {
    if (!cfg) return;
    searching = field === 'search';
    edit = searching ? undefined : field as keyof Config | 'ignore';
    const instruction = field === 'search'
      ? '  Filter files — Enter apply, Esc cancel'
      : `  ${field} — Enter apply, Esc cancel${field === 'targets' ? ' (space-separated target names)' : ''}`;
    inputValue = field === 'search' ? search : field === 'ignore' ? '' : field === 'targets' ? cfg.targets.join(' ') : String(cfg[field as keyof Config]);
    editing = true;
    prompt.content = `${instruction}  │  ${inputValue}`;
  }

  function cancel() {
    edit = undefined;
    searching = false;
    editing = false;
    prompt.content = '';
    draw();
  }

  function applyInput() {
    if (!cfg) return;
    const value = inputValue;
    if (searching) { search = value; filter(); selected = 0; cancel(); return; }
    const next = structuredClone(cfg);
    if (edit === 'ignore') { if (value.trim()) next.ignores.push(value.trim()); }
    else if (edit === 'targets') next.targets = value.trim().split(/\s+/).filter(Boolean);
    else if (edit === 'interval' || edit === 'debounce') {
      if (!/^\d+$/.test(value)) { prompt.content = '  Enter a whole number of milliseconds'; return; }
      next[edit] = Number(value);
    } else if (edit === 'makefile' || edit === 'exec') next[edit] = value;
    send({ cmd: 'config', config: next });
    cancel();
  }

  renderer.keyInput.on('keypress', (key: KeyEvent) => {
    const name = String(key.name).toLowerCase();
    if (key.ctrl && name === 'c') { key.preventDefault(); quit(); return; }
    if (editing) {
      // A fast Escape followed by Tab can be parsed as one Alt-Tab sequence
      // by the test renderer and by some terminals. Treat both intents here.
      if (key.sequence.includes('\x1b') && key.sequence.includes('\t')) { cancel(); page = (page + 1) % 3; draw(); return; }
      if (name === 'escape') { key.preventDefault(); cancel(); return; }
      if (name === 'enter' || name === 'return') { key.preventDefault(); applyInput(); return; }
      if (name === 'backspace') { inputValue = inputValue.slice(0, -1); prompt.content = `  Editing  │  ${inputValue}`; return; }
      if (!key.ctrl && !key.meta && !key.super && !key.hyper && key.sequence && !/[\x00-\x1f\x7f-\x9f]/.test(key.sequence)) {
        inputValue += key.sequence;
        prompt.content = `  Editing  │  ${inputValue}`;
      }
      return;
    }
    if (name === 'q') { quit(); return; }
    if (name === 'tab') { page = (page + 1) % 3; draw(); return; }
    if (name === 'up' || name === 'down') {
      const n = name === 'up' ? -1 : 1;
      if (page === 0) selected += n;
      else if (page === 1) logOffset -= n;
      else ruleOffset = Math.max(0, Math.min(ruleOffset + n, (cfg?.ignores.length ?? 0) + 3));
      draw(); return;
    }
    if (name === '/') { begin('search'); return; }
    const commands: Record<string, string> = { a: 'arm', b: 'build', p: 'pause', k: 'stop', s: 'save' };
    if (commands[name]) { send({ cmd: commands[name] }); return; }
    const edits: Record<string, keyof Config | 'ignore'> = { t: 'targets', e: 'exec', m: 'makefile', g: 'ignore', i: 'interval', d: 'debounce' };
    if (edits[name]) { begin(edits[name]); return; }
    if (name === 'c' && page === 2 && cfg) { send({ cmd: 'config', config: { ...cfg, ignores: [] } }); return; }
    if (name === 'x' && page === 0 && cfg && files[selected]) {
      const f = files[selected]!;
      const next = structuredClone(cfg);
      if (f.ignored) {
        const index = next.ignores.indexOf(f.path.replace(/[?*\[\]{}]/g, '[$&]'));
        if (index < 0) { message = 'Ignored by a glob: edit rules with g or clear them with c in RULES.'; draw(); return; }
        next.ignores.splice(index, 1);
      } else next.ignores.push(f.path.replace(/[?*\[\]{}]/g, '[$&]'));
      send({ cmd: 'config', config: next });
    }
  });

  const resize = () => draw();
  renderer.on('resize', resize);
  draw();

  return {
    receive(v: any) {
      if (v.type === 'ready') { cfg = v.config; allFiles = v.files; filter(); rootPath = v.root; active = v.active; }
      if (v.type === 'config') { cfg = v.config; allFiles = v.files; filter(); message = 'Rules applied in memory. Press s to save.'; }
      if (v.type === 'files') { allFiles = v.files; filter(); }
      if (v.type === 'status') { message = v.message; if (typeof v.active === 'boolean') active = v.active; if (typeof v.paused === 'boolean') paused = v.paused; if (typeof v.building === 'boolean') building = v.building; log(`› ${v.message}`); }
      if (v.type === 'change') log(`${v.kind.toUpperCase()}  ${v.path}`);
      if (v.type === 'log') log(`[${v.source}] ${v.text}`);
      draw();
    },
    fail(text: string) { message = text; log(text); draw(); },
  };
}
