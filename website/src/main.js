document.documentElement.classList.add('js');
const header = document.querySelector('.site-head');
const updateHeaderEdge = () => header?.classList.toggle('scrolled', window.scrollY > 0);
updateHeaderEdge();
window.addEventListener('scroll', updateHeaderEdge, { passive: true });

for (const [index, button] of [...document.querySelectorAll('.faq button')].entries()) {
  const answer = document.getElementById(button.getAttribute('aria-controls'));
  const expanded = index === 0;
  button.setAttribute('aria-expanded', String(expanded));
  answer.inert = !expanded;
  button.addEventListener('click', () => {
    const next = button.getAttribute('aria-expanded') !== 'true';
    button.setAttribute('aria-expanded', String(next));
    answer.inert = !next;
  });
}
for (const button of document.querySelectorAll('[data-copy]')) {
  button.addEventListener('click', async () => {
    const address = document.getElementById(button.dataset.copy);
    const status = button.closest('.copybox').nextElementSibling;
    try {
      await navigator.clipboard.writeText(address.textContent);
      status.textContent = 'Email address copied.';
    } catch {
      address.focus();
      const selection = window.getSelection();
      const range = document.createRange();
      range.selectNodeContents(address);
      selection.removeAllRanges();
      selection.addRange(range);
      status.textContent = 'Email address selected. Copy it with Ctrl+C or your device’s copy command.';
    }
  });
}

if ('IntersectionObserver' in window && !window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
  const observer = new IntersectionObserver(entries => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      entry.target.classList.add('reveal-visible');
      observer.unobserve(entry.target);
    }
  }, { threshold: 0.08 });
  for (const section of document.querySelectorAll('.section, .closing')) {
    if (section.getBoundingClientRect().top < window.innerHeight) continue;
    section.classList.add('reveal-pending');
    observer.observe(section);
  }
}
