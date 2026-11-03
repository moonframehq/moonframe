// Both web surfaces use the catalog generated from the real Luau files.
(() => {
  const base = new URL('./', document.currentScript.src);
  const all = window.PATTERNS || [];
  const esc = s => String(s).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const sourceURL = p => `https://github.com/moonframehq/moonframe/blob/main/patterns/app/components/${encodeURIComponent(p.id)}.luau`;
  const shot = (p, dark = false) => `<img src="${new URL(`previews/${p.id}${dark ? '-dark' : ''}.png`, base)}" alt="${esc(p.title)} rendered by Moonframe" loading="lazy">`;
  const fallback = '<p class="pb-fallback">Explore the live preview in the native browser.<br><code>just patterns</code></p>';
  function imageFallbacks(root) {
    root.querySelectorAll('img').forEach(img => img.addEventListener('error', () => { img.outerHTML = fallback; }, {once:true}));
  }
  const home = document.querySelector('#patterns-home');
  if (home && all.length) {
    const feature = all.find(p => p.id === 'workspace-shell') || all[0];
    const docs = 'https://docs.moonframe.dev/patterns/';
    home.innerHTML = `<div class="ph-intro"><div><p class="pb-eyebrow">The pattern book</p><h2>Build on good decisions.</h2><p>Between a component and a whole app: reusable compositions, with the reasoning left in. Find the pattern for what you’re making.</p></div><div class="ph-edition"><strong>01—${String(all.length).padStart(2,'0')}</strong>The first collection<br>Native by nature. Yours by design.</div></div>
      <article class="ph-feature"><div class="ph-copy"><span class="pb-eyebrow">${esc(feature.category)} / Essential pattern</span><h3>A home for complex work.</h3><p>${esc(feature.when)}</p><a class="pb-button" href="${docs}#${feature.id}">Explore ${esc(feature.title.toLowerCase())} →</a></div><div class="pb-shot">${shot(feature)}</div></article>
      ${all.filter(p => p !== feature).map((p,i) => `<a class="ph-row" href="${docs}#${p.id}"><span>${String(i+2).padStart(2,'0')}</span><div><strong>${esc(p.title)}</strong><p>${esc(p.description)}</p></div><small>${esc(p.category)}</small><span>→</span></a>`).join('')}
      <div class="ph-foot"><span>Browse alongside your app: <code>just patterns</code></span><a class="pb-button" href="${docs}">Browse all patterns →</a></div>`;
    imageFallbacks(home);
  }
  const root = document.querySelector('#pattern-browser');
  if (!root) return;
  const search = document.querySelector('#pb-search');
  const content = document.querySelector('#pb-content');
  const categories = ['All patterns', ...new Set(all.map(p => p.category).sort())];
  const state = {category:'All patterns', query:'', code:'example', dark:matchMedia('(prefers-color-scheme: dark)').matches};
  function selected() { return all.find(p => `#${p.id}` === location.hash); }
  function filtered() {
    const terms = state.query.toLowerCase().trim().split(/\s+/).filter(Boolean);
    return all.filter(p => (state.category === 'All patterns' || p.category === state.category) &&
      terms.every(term => `${p.title} ${p.id} ${p.category} ${p.description} ${p.parts.join(' ')}`.toLowerCase().includes(term)));
  }
  function notify(message) {
    document.querySelector('#pb-status').textContent = message;
    clearTimeout(notify.timer);
    notify.timer = setTimeout(() => { document.querySelector('#pb-status').textContent = ''; }, 3000);
  }
  function list() {
    const shown = filtered();
    return `<div class="pb-intro"><div><p class="pb-eyebrow">The considered middle</p><h1>Good UI, already composed.</h1><p class="pb-lede">Thoughtful starting points, built from real components. Explore the preview, read the code, and make them yours.</p></div><span class="pb-edition">COLLECTION 001</span></div>
      <div class="pb-grid-heading"><strong>${esc(state.category)}</strong><span>${shown.length} ${shown.length === 1 ? 'pattern' : 'patterns'}</span></div>
      <div class="pb-grid">${shown.map(p => `<a class="pb-tile" href="#${p.id}"><div class="pb-shot">${shot(p,state.dark)}</div><div class="pb-tile-head"><h3>${esc(p.title)}</h3><span class="pb-tag">${esc(p.category)}</span></div><p>${esc(p.description)}</p></a>`).join('')}</div>
      ${!shown.length ? '<p class="pb-empty">No patterns match. Try a different category or search.</p>' : ''}`;
  }
  function detail(p) {
    const source = state.code === 'source' ? p.source : p.example;
    return `<div class="pb-breadcrumb"><a href="#" data-back>← All patterns</a><span>${esc(p.category)} / ${esc(p.id)}</span></div>
      <div class="pb-title"><h1>${esc(p.title)}</h1><p class="pb-lede">${esc(p.description)}</p></div>
      <div class="pb-tools"><div class="pb-tabs" aria-label="Code to show"><button data-code="example" aria-pressed="${state.code==='example'}">Example</button><button data-code="source" aria-pressed="${state.code==='source'}">Pattern source</button></div><button class="pb-button" id="pb-copy">Copy ${state.code==='source'?'pattern':'example'}</button></div>
      <div class="pb-workbench"><div class="pb-preview">${shot(p,state.dark)}<small>Native screenshot · sample data<br>Interact with this pattern in <code>just patterns</code></small></div><div class="pb-source"><div class="pb-source-head">${state.code==='source'?`components/${p.id}.luau`:`mods/preview-${p.id}/main.luau`}</div><pre tabindex="0" aria-label="Luau source"><code>${esc(source)}</code></pre></div></div>
      <div class="pb-explanation"><div><h3>Why it works</h3><p>${esc(p.when)}</p></div><div><h3>Built from</h3><div class="pb-parts">${p.parts.map(part=>`<a href="https://github.com/moonframehq/moonframe/blob/main/app/components/${part}.luau">${esc(part)}</a>`).join('')}</div></div></div>
      <p class="pb-install">Copy <a href="${sourceURL(p)}">${p.id}.luau</a> into your app’s <code>components/</code> folder, then use the example in a mod. Styles live with the pattern; restyle its classes in <code>style.luau</code>. <a href="#using-patterns">Integration guide ↓</a></p>`;
  }
  function render() {
    document.body.classList.toggle('pb-dark',state.dark);
    document.querySelector('#pb-theme').setAttribute('aria-pressed',String(state.dark));
    document.querySelector('#pb-categories').innerHTML = categories.map(cat => `<button class="pb-category" data-category="${esc(cat)}" aria-pressed="${state.category===cat}">${esc(cat)}<span>${cat==='All patterns'?all.length:all.filter(p=>p.category===cat).length}</span></button>`).join('');
    const p = selected();
    content.innerHTML = p ? detail(p) : list();
    document.title = p ? `${p.title} — Moonframe Patterns` : 'Patterns — Moonframe Docs';
    imageFallbacks(content);
  }
  function browse() { history.replaceState(null,'',location.pathname+location.search); render(); }
  search.addEventListener('input',() => { state.query = search.value; browse(); });
  root.addEventListener('click',async e => {
    const category = e.target.closest('[data-category]');
    const code = e.target.closest('[data-code]');
    if (category) { state.category = category.dataset.category; browse(); }
    else if (code) { state.code = code.dataset.code; render(); content.querySelector(`[data-code="${state.code}"]`).focus(); }
    else if (e.target.closest('[data-back]')) { e.preventDefault(); browse(); }
    else if (e.target.closest('#pb-copy')) {
      const p = selected();
      try { await navigator.clipboard.writeText(state.code==='source'?p.source:p.example); notify('Copied the actual Luau source.'); }
      catch { notify('Clipboard unavailable. Select and copy the code instead.'); }
    }
  });
  document.querySelector('#pb-theme').addEventListener('click',()=>{state.dark=!state.dark;render();});
  window.addEventListener('hashchange',()=>{if(location.hash!=='#using-patterns'){render();content.focus();}});
  document.addEventListener('keydown',e=>{
    if(e.key==='/'&&!['INPUT','TEXTAREA'].includes(document.activeElement.tagName)){e.preventDefault();search.focus();}
    if(e.key==='Escape'&&selected()){browse();search.focus();}
  });
  render();
})();
