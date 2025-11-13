// Lightweight JSON Schema renderer for docs
// Supports object properties, arrays, primitives, and descriptions
function renderSchema(schema, mount) {
  const container = document.createElement('div');
  container.className = 'schema';

  const render = (sch, parent) => {
    if (!sch) return;
    const type = sch.type || (Array.isArray(sch.type) ? sch.type.join('|') : 'any');
    const desc = sch.description || '';
    const root = document.createElement('div');
    root.className = 'prop';
    const keySpan = document.createElement('span');
    keySpan.className = 'key';
    keySpan.textContent = sch.title || '(root)';
    const typeSpan = document.createElement('span');
    typeSpan.className = 'type';
    typeSpan.textContent = `:${type}`;
    const descSpan = document.createElement('span');
    descSpan.className = 'desc';
    descSpan.textContent = desc ? `– ${desc}` : '';
    root.appendChild(keySpan);
    root.appendChild(typeSpan);
    root.appendChild(descSpan);
    parent.appendChild(root);

    // Object properties
    if (sch.type === 'object' && sch.properties) {
      const nested = document.createElement('div');
      nested.className = 'nested';
      Object.entries(sch.properties).forEach(([k, v]) => {
        const child = { ...v, title: k };
        render(child, nested);
      });
      parent.appendChild(nested);
    }
    // Array items
    if (sch.type === 'array' && sch.items) {
      const nested = document.createElement('div');
      nested.className = 'nested';
      const child = { ...sch.items, title: sch.items.title || '(items)' };
      render(child, nested);
      parent.appendChild(nested);
    }
  };

  render(schema, container);
  mount.innerHTML = '';
  mount.appendChild(container);
}