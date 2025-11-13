// basic in-page search filter for headings
document.addEventListener('DOMContentLoaded', () => {
  const input = document.getElementById('searchInput');
  const sections = Array.from(document.querySelectorAll('article'));

  input?.addEventListener('input', () => {
    const q = (input.value || '').toLowerCase().trim();
    sections.forEach(sec => {
      const text = sec.textContent?.toLowerCase() || '';
      const match = q.length === 0 || text.includes(q);
      sec.style.display = match ? '' : 'none';
    });
  });
});