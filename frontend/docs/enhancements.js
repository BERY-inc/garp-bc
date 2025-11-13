// Enhancements: left-nav highlight on scroll, deep-linking for headings, and lunr search across pages
document.addEventListener('DOMContentLoaded', () => {
  // 1) Deep-link anchors for headings and ensure ids
  const makeId = (text) => text.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/(^-|-$)/g, '');
  const addAnchors = () => {
    const headings = document.querySelectorAll('article h1, article h2, article h3');
    headings.forEach(h => {
      if (!h.id) h.id = makeId(h.textContent || 'section');
      const a = document.createElement('a');
      a.href = `#${h.id}`;
      a.className = 'heading-anchor';
      a.textContent = '#';
      if (!h.querySelector('.heading-anchor')) h.appendChild(a);
    });
  };
  addAnchors();

  // Copy buttons on code blocks
  const initCopyButtons = () => {
    const pres = Array.from(document.querySelectorAll('pre'));
    pres.forEach(pre => {
      if (pre.querySelector('.copy-btn')) return;
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'copy-btn';
      btn.textContent = 'Copy';
      btn.addEventListener('click', async () => {
        const code = pre.querySelector('code');
        const text = (code ? code.textContent : pre.textContent || '').trim();
        try {
          await navigator.clipboard.writeText(text);
          btn.textContent = 'Copied';
          setTimeout(() => { btn.textContent = 'Copy'; }, 1500);
        } catch (_) {}
      });
      pre.appendChild(btn);
    });
  };
  initCopyButtons();

  // 2) Left-nav active highlight on scroll using IntersectionObserver
  const navLinks = Array.from(document.querySelectorAll('.nav a[href^="#"]'));
  const sections = Array.from(document.querySelectorAll('article[id]'));
  const linkById = new Map(navLinks.map(a => [a.getAttribute('href').slice(1), a]));
  let currentId = null;
  const observer = new IntersectionObserver((entries) => {
    entries.forEach(entry => {
      const id = entry.target.id;
      if (entry.isIntersecting) {
        currentId = id;
        navLinks.forEach(a => a.classList.remove('active'));
        const link = linkById.get(id);
        if (link) link.classList.add('active');
        // update hash without scrolling
        history.replaceState(null, '', `#${id}`);
      }
    });
  }, { rootMargin: '0px 0px -60% 0px', threshold: 0.1 });
  sections.forEach(sec => observer.observe(sec));

  // 2b) Generate Method Page Table of Contents if container exists
  const tocEl = document.getElementById('methodToc');
  if (tocEl) {
    const article = document.querySelector('section.content article');
    const heads = article ? Array.from(article.querySelectorAll('h1, h2, h3')) : [];
    const ul = document.createElement('ul');
    const title = document.createElement('h4');
    title.textContent = 'On this page';
    tocEl.appendChild(title);
    heads.forEach(h => {
      const li = document.createElement('li');
      const a = document.createElement('a');
      const id = h.id || makeId(h.textContent || 'section');
      h.id = id;
      a.href = `#${id}`;
      a.textContent = h.textContent || id;
      li.appendChild(a);
      ul.appendChild(li);
    });
    tocEl.appendChild(ul);
  }

  // Collapsible sidebar sections, grouping Docs and Methods
  const initSidebarSections = () => {
    const nav = document.querySelector('.sidebar .nav');
    if (!nav) return;
    const existingSections = nav.querySelector('.sidebar-section');
    const links = Array.from(nav.querySelectorAll('a'));
    if (existingSections || !links.length) return;
    const makeSection = (title, items, key) => {
      const section = document.createElement('div');
      section.className = 'sidebar-section';
      const header = document.createElement('button');
      header.className = 'sidebar-section-header';
      header.type = 'button';
      const label = document.createElement('span');
      label.textContent = title;
      const chev = document.createElement('span');
      chev.className = 'chevron';
      header.appendChild(label);
      header.appendChild(chev);
      const content = document.createElement('div');
      content.className = 'sidebar-section-content';
      items.forEach(a => content.appendChild(a));
      const open = localStorage.getItem(`sidebar:${key}`) !== 'closed';
      header.setAttribute('aria-expanded', open ? 'true' : 'false');
      content.style.display = open ? 'block' : 'none';
      header.addEventListener('click', () => {
        const expanded = header.getAttribute('aria-expanded') === 'true';
        header.setAttribute('aria-expanded', expanded ? 'false' : 'true');
        content.style.display = expanded ? 'none' : 'block';
        localStorage.setItem(`sidebar:${key}`, expanded ? 'closed' : 'open');
      });
      section.appendChild(header);
      section.appendChild(content);
      return section;
    };
    const docsItems = links.filter(a => !a.getAttribute('href').includes('json-rpc/'));
    const methodItems = links.filter(a => a.getAttribute('href').includes('json-rpc/'));
    nav.innerHTML = '';
    nav.appendChild(makeSection('Docs', docsItems, 'docs'));
    nav.appendChild(makeSection('Methods', methodItems, 'methods'));
  };
  initSidebarSections();

  // Mobile topbar & sidebar overlay toggle
  const initMobileTopbar = () => {
    if (document.querySelector('.topbar')) return;
    const top = document.createElement('div');
    top.className = 'topbar';
    const btn = document.createElement('button');
    btn.className = 'menu-button';
    btn.type = 'button';
    btn.setAttribute('aria-label', 'Toggle sidebar');
    btn.textContent = 'Menu';
    const title = document.createElement('div');
    title.className = 'topbar-title';
    title.textContent = 'GARP Docs';
    top.appendChild(btn);
    top.appendChild(title);
    document.body.appendChild(top);
    const overlay = document.createElement('div');
    overlay.className = 'sidebar-overlay';
    document.body.appendChild(overlay);
    const toggle = (on) => {
      const willOpen = on === undefined ? !document.body.classList.contains('sidebar-open') : !!on;
      document.body.classList.toggle('sidebar-open', willOpen);
    };
    btn.addEventListener('click', () => toggle());
    overlay.addEventListener('click', () => toggle(false));
  };
  initMobileTopbar();

  // Breadcrumbs on method pages
  const initBreadcrumbs = () => {
    const content = document.querySelector('section.content');
    const titleEl = content ? content.querySelector('article h1') : null;
    if (!content || !titleEl || document.querySelector('.breadcrumbs')) return;
    const isMethod = location.pathname.includes('/json-rpc/');
    if (!isMethod) return;
    const nav = document.createElement('nav');
    nav.className = 'breadcrumbs';
    const aHome = document.createElement('a');
    aHome.href = '../index.html';
    aHome.textContent = 'Docs';
    const sep1 = document.createTextNode(' / ');
    const aCat = document.createElement('a');
    aCat.href = './';
    aCat.textContent = 'JSON-RPC';
    const sep2 = document.createTextNode(' / ');
    const span = document.createElement('span');
    span.textContent = titleEl.textContent || '';
    nav.appendChild(aHome);
    nav.appendChild(sep1);
    nav.appendChild(aCat);
    nav.appendChild(sep2);
    nav.appendChild(span);
    content.insertBefore(nav, content.firstChild);
  };
  initBreadcrumbs();

  // Convert Notes/Warning/Tip sections into callouts
  const initCallouts = () => {
    const article = document.querySelector('section.content article');
    if (!article) return;
    const heads = Array.from(article.querySelectorAll('h2, h3'));
    heads.forEach(h => {
      const t = (h.textContent || '').toLowerCase();
      let kind = null;
      if (t.includes('note')) kind = 'info';
      else if (t.includes('warning')) kind = 'warn';
      else if (t.includes('tip')) kind = 'tip';
      if (!kind) return;
      const wrap = document.createElement('div');
      wrap.className = `callout ${kind}`;
      let next = h.nextElementSibling;
      const toMove = [];
      while (next && !/^H[1-6]$/.test(next.tagName)) {
        toMove.push(next);
        next = next.nextElementSibling;
      }
      toMove.forEach(el => wrap.appendChild(el));
      h.replaceWith(wrap);
    });
  };
  initCallouts();

  // 3) Search: prefer Algolia DocSearch if available, otherwise Lunr
  const initSearchWithLunr = () => {
    const resultsEl = document.getElementById('searchResults');
    const input = document.getElementById('searchInput');
    if (!input || !resultsEl) return;
    const docs = [];
    // Index current page sections
    sections.forEach(sec => {
      docs.push({
        id: sec.id,
        title: sec.querySelector('h1, h2, h3')?.textContent || sec.id,
        url: `#${sec.id}`,
        content: sec.textContent || ''
      });
    });
    // Known external method pages to index
    const methodPages = [
      { url: 'json-rpc/getBlock.html', title: 'getBlock' },
      { url: 'json-rpc/simulateTransaction.html', title: 'simulateTransaction' },
      { url: 'json-rpc/sendTransaction.html', title: 'sendTransaction' },
      { url: 'json-rpc/getBalance.html', title: 'getBalance' },
      { url: 'json-rpc/getSlotLeader.html', title: 'getSlotLeader' },
    ];
    const fetchText = async (url) => {
      try {
        const res = await fetch(url);
        if (!res.ok) return '';
        const html = await res.text();
        const div = document.createElement('div');
        div.innerHTML = html;
        const main = div.querySelector('main') || div;
        return (main.textContent || '').replace(/\s+/g, ' ').trim();
      } catch (_) { return ''; }
    };
    const buildIndex = async () => {
      for (const mp of methodPages) {
        const content = await fetchText(mp.url);
        docs.push({ id: mp.url, title: mp.title, url: mp.url, content });
      }
      if (typeof lunr !== 'function') {
        return { idx: null };
      }
      const idx = lunr(function () {
        this.ref('id');
        this.field('title');
        this.field('content');
        docs.forEach(d => this.add(d));
      });
      return { idx };
    };
    const renderResults = (items) => {
      resultsEl.innerHTML = '';
      if (!items.length) return;
      items.slice(0, 10).forEach(it => {
        const d = docs.find(x => x.id === it.ref);
        if (!d) return;
        const el = document.createElement('div');
        el.className = 'search-result';
        const a = document.createElement('a');
        a.href = d.url;
        a.textContent = d.title;
        const p = document.createElement('p');
        p.textContent = (d.content || '').slice(0, 180) + (d.content.length > 180 ? '…' : '');
        el.appendChild(a);
        el.appendChild(p);
        resultsEl.appendChild(el);
      });
    };
    buildIndex().then(({ idx }) => {
      if (!idx) return; // Skip search on pages without lunr
      input.addEventListener('input', () => {
        const q = (input.value || '').trim();
        if (!q) { resultsEl.innerHTML = ''; return; }
        const res = idx.search(q);
        renderResults(res);
      });
    });
  };
  const initSearchProvider = () => {
    const input = document.getElementById('searchInput');
    if (!input) return;
    if (window.docsearch && window.DOCSEARCH_CONFIG) {
      try {
        // Minimal docsearch setup; assumes docsearch.js/css included externally
        window.docsearch(window.DOCSEARCH_CONFIG);
        return;
      } catch (_) { /* fall back to lunr */ }
    }
    initSearchWithLunr();
  };
  initSearchProvider();
});