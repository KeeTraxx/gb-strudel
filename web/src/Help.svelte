<script>
  import { marked } from 'marked';
  import readme from '../../README.md?raw';

  // The syntax reference is the README's, so the two can't drift: everything
  // from "## Song files" up to (not including) "## Instruments", which the
  // sidebar already lists.
  const FIRST = '## Song files';
  const END = '## Instruments';
  const README_URL = 'https://github.com/KeeTraxx/gb-strudel#song-files';

  const start = readme.indexOf(FIRST);
  const end = readme.indexOf(END, start);
  if (start < 0 || end < 0) {
    console.error(`help: README no longer has "${FIRST}" followed by "${END}"`);
  }
  // Our own README, so rendering it as HTML is safe.
  const html = start >= 0 && end >= 0 ? marked.parse(readme.slice(start, end)) : '';

  let dialog;

  export function open() {
    dialog.showModal();
  }
</script>

<!-- Clicking the backdrop (the dialog element itself, outside .body) closes it. -->
<dialog bind:this={dialog} onclick={(e) => e.target === dialog && dialog.close()}>
  <div class="body">
    <button class="close" onclick={() => dialog.close()} aria-label="Close">✕</button>
    {#if html}
      {@html html}
    {/if}
    <p>
      Full documentation: <a href={README_URL} target="_blank" rel="noreferrer">README</a>
    </p>
  </div>
</dialog>

<style>
  dialog {
    width: min(48rem, 90vw);
    max-height: 85vh;
    padding: 0;
    border: 1px solid #3d633d;
    border-radius: 6px;
    background: #0f1a0f;
    color: inherit;
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 0.6);
  }
  .body {
    position: relative;
    padding: 0.5rem 1.5rem 1rem;
    line-height: 1.5;
  }
  .close {
    position: sticky;
    top: 0.5rem;
    float: right;
    font: inherit;
    color: inherit;
    background: #1c2e1c;
    border: 1px solid #3d633d;
    border-radius: 4px;
    cursor: pointer;
  }
  .body :global(h2),
  .body :global(h3) {
    color: #9bbc0f;
  }
  .body :global(pre) {
    padding: 0.75rem;
    overflow-x: auto;
    background: #1c2e1c;
    border-radius: 4px;
  }
  .body :global(code) {
    font: inherit;
    color: #e0f0c0;
  }
  .body :global(table) {
    border-collapse: collapse;
    margin: 0.5rem 0;
  }
  .body :global(th),
  .body :global(td) {
    padding: 0.2rem 0.6rem;
    border: 1px solid #2c4a2c;
    text-align: left;
  }
  .body :global(a) {
    color: #9bbc0f;
  }
</style>
