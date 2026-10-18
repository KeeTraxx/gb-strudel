<script>
  import { onDestroy } from 'svelte';
  import { compile, instrumentNames, sampleRate } from './pkg/gb_strudel_wasm.js';
  import { CHANNELS, Player } from './player.js';
  import Help from './Help.svelte';

  // Every song in ../songs, keyed by file name without the extension.
  const examples = Object.fromEntries(
    Object.entries(
      import.meta.glob('../../songs/*.gbs', { query: '?raw', import: 'default', eager: true }),
    ).map(([path, src]) => [path.split('/').pop().replace(/\.gbs$/, ''), src]),
  );
  const DEFAULT_EXAMPLE = 'coffee_break' in examples ? 'coffee_break' : Object.keys(examples)[0];
  const STORAGE_KEY = 'gb-strudel:source';

  const names = instrumentNames();
  const banks = [
    { label: 'pulse1 / pulse2', names: names.slice(0, 15) },
    { label: 'wave', names: names.slice(15, 30) },
    { label: 'noise', names: names.slice(30, 45) },
  ];

  let source = $state(localStorage.getItem(STORAGE_KEY) ?? examples[DEFAULT_EXAMPLE]);
  let example = $state('');
  let error = $state('');
  let info = $state(null);
  let playing = $state(false);
  let position = $state(0);
  let duration = $state(0);
  let help;
  const player = new Player();
  player.setRepeat(true);
  let muted = $state([...player.muted]);
  let repeat = $state(player.repeat);
  // The last song that compiled. It owns wasm memory, so it is freed when
  // replaced.
  let compiled = null;

  // Compile on every edit (it is cheap) so errors show while typing.
  $effect(() => {
    localStorage.setItem(STORAGE_KEY, source);
    try {
      const next = compile(source);
      compiled?.free();
      compiled = next;
      info = { name: next.name, bars: next.bars };
      error = '';
    } catch (e) {
      error = e.message ?? String(e);
    }
  });

  async function play() {
    if (!compiled || error) return;
    // Re-playing while a song runs keeps the position, so edits can be heard
    // in place.
    const offset = player.playing ? player.position : 0;
    await player.play(compiled.render(), sampleRate(), offset, () => (playing = false));
    duration = player.duration;
    playing = true;
  }

  function stop() {
    player.stop();
    playing = false;
  }

  function toggleMute(ch) {
    muted[ch] = !muted[ch];
    player.setMuted(ch, muted[ch]);
  }

  function toggleRepeat() {
    repeat = !repeat;
    player.setRepeat(repeat);
  }

  function loadExample() {
    if (!example) return;
    source = examples[example];
    example = '';
  }

  function downloadUge() {
    if (!compiled) return;
    const blob = new Blob([compiled.uge()], { type: 'application/octet-stream' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = `${slug(info?.name) || 'song'}.uge`;
    a.click();
    URL.revokeObjectURL(a.href);
  }

  function slug(name) {
    return (name ?? '')
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '_')
      .replace(/^_|_$/g, '');
  }

  // Strudel's keys: ctrl+enter (re)plays, ctrl+. stops.
  function onKeydown(e) {
    if (!(e.ctrlKey || e.metaKey)) return;
    if (e.key === 'Enter') {
      e.preventDefault();
      play();
    } else if (e.key === '.') {
      e.preventDefault();
      stop();
    }
  }

  let frame = requestAnimationFrame(function tick() {
    position = player.position;
    frame = requestAnimationFrame(tick);
  });
  onDestroy(() => {
    cancelAnimationFrame(frame);
    player.stop();
  });

  function time(seconds) {
    const s = Math.floor(seconds);
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
  }

  let bar = $derived(
    info && duration > 0 ? Math.min(Math.floor((position / duration) * info.bars) + 1, info.bars) : 1,
  );
</script>

<svelte:window onkeydown={onKeydown} />

<header>
  <h1>gb-strudel</h1>
  <div class="transport">
    <button onclick={play} disabled={!!error} title="ctrl+enter">
      {playing ? '↻ Update' : '▶ Play'}
    </button>
    <button onclick={stop} disabled={!playing} title="ctrl+.">■ Stop</button>
    <button class:on={repeat} onclick={toggleRepeat}>⟳ Repeat</button>
    <span class="status">
      {time(position)} / {time(duration)} · bar {bar}/{info?.bars ?? 0}
    </span>
  </div>
  <div class="file">
    <select bind:value={example} onchange={loadExample}>
      <option value="">Load example…</option>
      {#each Object.keys(examples) as name}
        <option value={name}>{name}</option>
      {/each}
    </select>
    <button onclick={downloadUge} disabled={!!error}>⤓ .uge</button>
    <button onclick={() => help.open()}>? Help</button>
  </div>
</header>

<Help bind:this={help} />

<main>
  <section class="editor">
    <textarea bind:value={source} spellcheck="false" autocomplete="off"></textarea>
    <pre class="message" class:error>{error || `✓ ${info?.name || 'untitled'}, ${info?.bars} bars`}</pre>
  </section>

  <aside>
    <h2>Channels</h2>
    {#each CHANNELS as name, ch}
      <button class="channel" class:muted={muted[ch]} onclick={() => toggleMute(ch)}>
        <span class="dot">{muted[ch] ? '○' : '●'}</span>
        {name}
        <span class="state">{muted[ch] ? 'muted' : 'on'}</span>
      </button>
    {/each}

    <h2>Instruments</h2>
    {#each banks as bank}
      <h3>{bank.label}</h3>
      <ol>
        {#each bank.names as name}
          <li>{name}</li>
        {/each}
      </ol>
    {/each}
    <p class="hint">
      Preview is approximate: it ignores instrument envelopes.
      <a href="https://github.com/KeeTraxx/gb-strudel#readme">Song format</a>
    </p>
  </aside>
</main>

<style>
  :global(body) {
    margin: 0;
    background: #0f1a0f;
    color: #c4dfa0;
    font-family: ui-monospace, 'SF Mono', Menlo, Consolas, monospace;
    font-size: 14px;
  }
  :global(#app) {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 1rem;
    padding: 0.5rem 1rem;
    border-bottom: 1px solid #2c4a2c;
  }
  h1 {
    margin: 0;
    font-size: 1.1rem;
    color: #9bbc0f;
  }
  .transport,
  .file {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .file {
    margin-left: auto;
  }
  .status {
    color: #7a9a6a;
    font-variant-numeric: tabular-nums;
  }
  button,
  select {
    font: inherit;
    color: inherit;
    background: #1c2e1c;
    border: 1px solid #3d633d;
    border-radius: 4px;
    padding: 0.3rem 0.7rem;
    cursor: pointer;
  }
  button:hover:not(:disabled) {
    background: #2a442a;
  }
  button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  button.on {
    border-color: #9bbc0f;
    color: #9bbc0f;
  }
  main {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .editor {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  textarea {
    flex: 1;
    resize: none;
    border: 0;
    outline: 0;
    padding: 1rem;
    background: transparent;
    color: inherit;
    font: inherit;
    line-height: 1.5;
    tab-size: 4;
  }
  .message {
    margin: 0;
    padding: 0.5rem 1rem;
    border-top: 1px solid #2c4a2c;
    color: #7a9a6a;
    white-space: pre-wrap;
  }
  .message.error {
    color: #ff8a7a;
  }
  aside {
    width: 16rem;
    overflow-y: auto;
    padding: 0 1rem 1rem;
    border-left: 1px solid #2c4a2c;
  }
  h2 {
    font-size: 0.9rem;
    color: #9bbc0f;
    margin: 1rem 0 0.5rem;
  }
  h3 {
    font-size: 0.8rem;
    color: #7a9a6a;
    margin: 0.75rem 0 0.25rem;
  }
  .channel {
    display: flex;
    width: 100%;
    gap: 0.5rem;
    margin-bottom: 0.25rem;
    text-align: left;
    color: #9bbc0f;
  }
  .channel.muted {
    color: #ff8a7a;
  }
  .channel .state {
    margin-left: auto;
  }
  ol {
    margin: 0;
    padding-left: 2rem;
    font-size: 0.8rem;
  }
  .hint {
    font-size: 0.75rem;
    color: #7a9a6a;
  }
  a {
    color: #9bbc0f;
  }
</style>
